// Runs the runtime on a small core-only project and checks what it logs.
// Usage: node runtime/test/smoke.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");
const assert = require("assert");

const root = path.join(__dirname, "..", "..");
const core = path.join(root, "plugins", "xc.core", "blocks");
const block = (rel) => ({ source: fs.readFileSync(path.join(core, rel), "utf8"), file: path.join(core, rel) });

const bundle = {
  project: { name: "Smoke", mode: "test", settings: {} },
  blocks: {
    "core.on_start": block("events/on_start.js"),
    "core.on_signal": block("events/on_signal.js"),
    "core.on_error": block("events/on_error.js"),
    "core.emit_signal": block("flow/emit_signal.js"),
    "core.if": block("flow/if.js"),
    "core.check": block("flow/check.js"),
    "core.math": block("math/math.js"),
    "core.compare": block("logic/compare.js"),
    "core.log": block("debug/log.js"),
    "core.throw": block("debug/throw.js"),
    "core.increase_stored": block("data/increase_stored.js"),
    "core.wait": block("flow/wait.js"),
  },
  libs: [],
  workspaces: {
    main: {
      name: "Main",
      nodes: {
        a: { block: "core.on_start@1", at: [0, 0] },
        m: { block: "core.math@1", at: [0, 0], values: { a: 6, op: "multiply", b: 7 } },
        cmp: { block: "core.compare@1", at: [0, 0], values: { op: "eq", b: 42 } },
        if: { block: "core.if@1", at: [0, 0] },
        yes: { block: "core.log@1", at: [0, 0], values: { message: "answer ${result}", level: "success" } },
        no: { block: "core.log@1", at: [0, 0], values: { message: "wrong", level: "error" } },
        emit: { block: "core.emit_signal@1", at: [0, 0], values: { name: "ping", data: "hello" } },
        // Try Zone: fails twice (retry 1), then the Error output logs the message.
        t: { block: "core.throw@1", at: [0, 0], values: { message: "boom" }, zone: "z1" },
        caught: { block: "core.log@1", at: [0, 0], values: { level: "warn" }, pins: ["message"] },
        // Outside any zone: goes to On Error.
        t2: { block: "core.throw@1", at: [0, 0], values: { message: "uncaught one" } },
      },
      links: [
        "a.next > if.in",
        "m.result > cmp.a",
        // A wire into a name that is not a field fills that ${placeholder}.
        "m.result > yes.result",
        "cmp.result > if.condition",
        "if.true > yes.in",
        "if.false > no.in",
        "yes.next > emit.in",
        "emit.next > t.in",
        "z1.error > caught.in",
        "z1.message > caught.message",
        "caught.next > t2.in",
      ],
      zones: { z1: { title: "Risky", rect: [0, 0, 10, 10], retry: 1 } },
    },
    signals: {
      name: "Signals",
      nodes: {
        s: { block: "core.on_signal@1", at: [0, 0], values: { name: "ping" } },
        inc: { block: "core.increase_stored@1", at: [0, 0], values: { scope: "global", key: "pings", by: 2 } },
        log: { block: "core.log@1", at: [0, 0], values: { message: "signal ${data}", level: "info" } },
        e: { block: "core.on_error@1", at: [0, 0] },
        elog: { block: "core.log@1", at: [0, 0], values: { message: "on error: ${message} in ${block}", level: "info" } },
        // On Error failing itself is logged once, not handled again and again.
        eboom: { block: "core.throw@1", at: [0, 0], values: { message: "handler broke" } },
      },
      links: ["s.next > inc.in", "inc.next > log.in", "e.next > elog.in", "elog.next > eboom.in"],
    },
    // Check continues on True or False by itself.
    checks: {
      name: "Checks",
      nodes: {
        go: { block: "core.on_start@1", at: [0, 0] },
        range: { block: "core.check@1", at: [0, 0], values: { a: "12", op: "between", b: "10", c: 20 } },
        exact: { block: "core.check@1", at: [0, 0], values: { a: "Hello", op: "eq", b: "hello" } },
        ok: { block: "core.log@1", at: [0, 0], values: { message: "checks went true then false", level: "info" } },
        bad: { block: "core.log@1", at: [0, 0], values: { message: "check took the wrong way", level: "error" } },
      },
      links: ["go.next > range.in", "range.true > exact.in", "range.false > bad.in", "exact.true > bad.in", "exact.false > ok.in"],
    },
    // A Try Zone that times out stops at its next block, while a path next to
    // it (an old project's split path) keeps running outside the zone.
    timeouts: {
      name: "Timeouts",
      nodes: {
        go: { block: "core.on_start@1", at: [0, 0] },
        slow: { block: "core.wait@1", at: [0, 0], values: { seconds: 0.3 }, zone: "zt" },
        late: { block: "core.log@1", at: [0, 0], values: { message: "zone kept going", level: "error" }, zone: "zt" },
        gave: { block: "core.log@1", at: [0, 0], values: { message: "zone gave up", level: "info" } },
        side: { block: "core.wait@1", at: [0, 0], values: { seconds: 0.1 } },
        sideLog: { block: "core.log@1", at: [0, 0], values: { message: "side path ran", level: "info" } },
      },
      links: ["go.next > slow.in", "go.next > side.in", "slow.next > late.in", "zt.error > gave.in", "side.next > sideLog.in"],
      zones: { zt: { title: "Slow", rect: [0, 0, 10, 10], timeoutMs: 50 } },
    },
  },
};

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-smoke-"));
const bundlePath = path.join(dir, "bundle.json");
fs.writeFileSync(bundlePath, JSON.stringify(bundle));

const child = spawn(process.execPath, [path.join(root, "runtime", "circuit-runtime.js"), "--bundle", bundlePath, "--deps", dir, "--data", path.join(dir, "data"), "--debug"]);
let out = "";
child.stdout.on("data", (d) => (out += d));
child.stderr.on("data", (d) => (out += d));

setTimeout(() => {
  child.kill();
  const events = out
    .split(/\r?\n/)
    .filter((l) => l.startsWith("@@xc "))
    .map((l) => JSON.parse(l.slice(5)));
  const logs = events.filter((e) => e.t === "log").map((e) => `${e.level}: ${e.text}`);
  console.log(logs.join("\n"));

  const has = (s) => assert.ok(logs.some((l) => l.includes(s)), `expected a log containing "${s}"`);
  has("success: answer 42");
  has("info: signal hello");
  has("info: zone gave up");
  has("info: side path ran");
  has("info: checks went true then false");
  assert.ok(!logs.some((l) => l.includes("wrong way")), "Check takes the right way");
  assert.ok(!logs.some((l) => l.includes("zone kept going")), "a zone that timed out stops");
  has("warn: Main › Try Risky: boom · retry 1/1");
  has("warn: boom");
  has("info: on error: uncaught one in Throw Error");
  has("handler broke (in an On Error path)");
  assert.strictEqual(logs.filter((l) => l.includes("on error:")).length, 1, "On Error ran once");
  assert.ok(!logs.some((l) => l.includes("wrong")), "the false branch must not run");
  assert.ok(events.some((e) => e.t === "node" && e.node === "yes" && e.phase === "exit"), "node events are emitted");
  assert.ok(events.some((e) => e.t === "zone" && e.phase === "error"), "zone error event");

  setTimeout(() => {
    const store = JSON.parse(fs.readFileSync(path.join(dir, "data", "store.json"), "utf8"));
    assert.strictEqual(store.global.pings, 2);
    console.log("\nsmoke test passed");
  }, 50);
}, 1500);
