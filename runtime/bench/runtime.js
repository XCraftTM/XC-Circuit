// Benchmarks the runtime that runs bots: how fast blocks run, how fast
// stored data and signals are, how long a bot takes to start, and how much
// memory it uses. Each case runs twice: as the editor runs bots (Run, with
// --debug: every block reports when it starts and ends, so blocks glow) and
// as an exported bot.
// Usage: node runtime/bench/runtime.js   (writes target/bench/runtime.json)
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");

const root = path.join(__dirname, "..", "..");
const core = path.join(root, "plugins", "xc.core", "blocks");
const block = (rel) => ({ source: fs.readFileSync(path.join(core, rel), "utf8"), file: path.join(core, rel) });
const blocks = {
  "core.on_start": block("events/on_start.js"),
  "core.on_signal": block("events/on_signal.js"),
  "core.emit_signal": block("flow/emit_signal.js"),
  "core.repeat": block("flow/repeat.js"),
  "core.math": block("math/math.js"),
  "core.set_variable": block("variables/set_variable.js"),
  "core.increase_stored": block("data/increase_stored.js"),
  "core.log": block("debug/log.js"),
};

const ROUNDS = 10000;
// Start and end marks with the time, and at the end the memory in use.
const start = (id) => ({ [id]: { block: "core.log@1", at: [0, 0], values: { message: "mark start ${Date.now()}" } } });
const done = (id) => ({ [id]: { block: "core.log@1", at: [0, 0], values: { message: "mark done ${Date.now()} ${process.memoryUsage().rss}" } } });

// Repeat 10 000 times: five Set Variable blocks, each getting a Math result.
function flowCase() {
  const nodes = { go: { block: "core.on_start@1", at: [0, 0] }, ...start("s"), rep: { block: "core.repeat@1", at: [0, 0], values: { times: ROUNDS } }, ...done("d") };
  const links = ["go.next > s.in", "s.next > rep.in", "rep.done > d.in"];
  let prev = "rep.each";
  for (let k = 0; k < 5; k++) {
    nodes[`m${k}`] = { block: "core.math@1", at: [0, 0], values: { op: "multiply", b: 2 } };
    nodes[`v${k}`] = { block: "core.set_variable@1", at: [0, 0], values: { name: `x${k}`, scope: "run" } };
    links.push(`${prev} > v${k}.in`, `rep.count > m${k}.a`, `m${k}.result > v${k}.value`);
    prev = `v${k}.next`;
  }
  return { nodes, links, blocks: ROUNDS * 5, unit: "blocks" };
}

// Repeat 10 000 times: Increase Stored Value (saved data, written to disk in batches).
function storeCase() {
  return {
    nodes: {
      go: { block: "core.on_start@1", at: [0, 0] },
      ...start("s"),
      rep: { block: "core.repeat@1", at: [0, 0], values: { times: ROUNDS } },
      inc: { block: "core.increase_stored@1", at: [0, 0], values: { scope: "global", key: "count", by: 1 } },
      ...done("d"),
    },
    links: ["go.next > s.in", "s.next > rep.in", "rep.each > inc.in", "rep.done > d.in"],
    blocks: ROUNDS,
    unit: "updates",
  };
}

// Repeat 10 000 times: send a signal; On Signal handles each one.
function signalCase() {
  return {
    nodes: {
      go: { block: "core.on_start@1", at: [0, 0] },
      ...start("s"),
      rep: { block: "core.repeat@1", at: [0, 0], values: { times: ROUNDS } },
      emit: { block: "core.emit_signal@1", at: [0, 0], values: { name: "tick" }, pins: ["data"] },
      ...done("d"),
      on: { block: "core.on_signal@1", at: [0, 0], values: { name: "tick" } },
      v: { block: "core.set_variable@1", at: [0, 0], values: { name: "last", scope: "global" } },
    },
    links: ["go.next > s.in", "s.next > rep.in", "rep.each > emit.in", "rep.count > emit.data", "rep.done > d.in", "on.next > v.in", "on.data > v.value"],
    blocks: ROUNDS,
    unit: "signals",
  };
}

function run(ws, debug) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-bench-"));
  const bundle = { project: { name: "Bench", mode: "test", settings: {} }, blocks, libs: [], workspaces: { main: { name: "Main", ...ws } } };
  fs.writeFileSync(path.join(dir, "bundle.json"), JSON.stringify(bundle));
  const args = [path.join(root, "runtime", "circuit-runtime.js"), "--bundle", path.join(dir, "bundle.json"), "--deps", dir, "--data", path.join(dir, "data")];
  if (debug) args.push("--debug");
  const spawned = Date.now();
  const child = spawn(process.execPath, args);
  let out = "";
  let lines = 0;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error(`timed out:\n${out.slice(-2000)}`));
    }, 120000);
    let s = null;
    let d = null;
    let rest = "";
    const check = () => {
      if (s && d) {
        clearTimeout(timer);
        child.kill();
        // Removed once the bot has really stopped (it may still be saving).
        child.once("exit", () => {
          try {
            fs.rmSync(dir, { recursive: true, force: true, maxRetries: 5 });
          } catch {}
        });
        resolve({ startup: Number(s[1]) - spawned, ms: Number(d[1]) - Number(s[1]), rss: Number(d[2]), lines });
      }
    };
    child.stdout.on("data", (chunk) => {
      const text = chunk.toString();
      lines += text.split("\n").length - 1;
      // Only the marks matter, not every block event (lines can span chunks).
      const parts = (rest + text).split("\n");
      rest = parts.pop();
      for (const l of parts) {
        s = s || /mark start (\d+)/.exec(l);
        d = d || /mark done (\d+) (\d+)/.exec(l);
      }
      check();
    });
    child.stderr.on("data", (chunk) => (out = (out + chunk).slice(-4000)));
  });
}

(async () => {
  const results = [];
  const cases = [
    ["Blocks (Repeat × 5 Set Variable with Math)", flowCase()],
    ["Stored data (Increase Stored Value)", storeCase()],
    ["Signals (Emit Signal → On Signal)", signalCase()],
  ];
  console.log("| case | mode | total ms | per item (µs) | per second | memory (MB) | start (ms) |\n|---|---|---|---|---|---|---|");
  for (const [name, c] of cases) {
    for (const [mode, debug] of [["Run in the editor", true], ["Exported bot", false]]) {
      // Best of three, after one warm-up run.
      await run(c, debug);
      const tries = [];
      for (let i = 0; i < 3; i++) tries.push(await run(c, debug));
      const best = tries.reduce((a, b) => (b.ms < a.ms ? b : a));
      const per = (best.ms * 1000) / c.blocks;
      const perSecond = Math.round((c.blocks / Math.max(best.ms, 1)) * 1000);
      const row = { case: name, mode, items: c.blocks, unit: c.unit, total_ms: best.ms, per_item_us: Math.round(per * 10) / 10, per_second: perSecond, rss_mb: Math.round(best.rss / 1048576), startup_ms: Math.min(...tries.map((t) => t.startup)) };
      results.push(row);
      console.log(`| ${name} | ${mode} | ${row.total_ms} | ${row.per_item_us} | ${row.per_second.toLocaleString("en-US")} ${c.unit} | ${row.rss_mb} | ${row.startup_ms} |`);
    }
  }
  const out = path.join(root, "target", "bench");
  fs.mkdirSync(out, { recursive: true });
  fs.writeFileSync(path.join(out, "runtime.json"), JSON.stringify({ node: process.version, rows: results }, null, 2));
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
