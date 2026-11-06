// One-time timers: Schedule, On Scheduled, Cancel and List Scheduled in a
// running bot; a timer survives a restart; missed timers run when the bot
// starts, unless On Scheduled says not to.
// Usage: node runtime/test/timers.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const assert = require("assert");
const { spawn } = require("child_process");
const { Store, Timers } = require("../circuit-runtime.js");

const root = path.join(__dirname, "..", "..");
const blocks = {};
for (const sub of ["timers", "events", "debug"]) {
  const folder = path.join(root, "plugins", "xc.core", "blocks", sub);
  for (const f of fs.readdirSync(folder)) {
    const src = fs.readFileSync(path.join(folder, f), "utf8");
    blocks[require(path.join(folder, f)).id] = { source: src, file: path.join(folder, f) };
  }
}
const n = (block, values = {}) => ({ block: block + "@1", at: [0, 0], values });
const say = (message) => n("core.log", { message, level: "info" });

/** Runs a bot until `done(logs)` or 15 s, then stops it. Gives its logs. */
function runBot(dir, nodes, links, done) {
  const bundle = { project: { name: "Timers", mode: "test", settings: {}, data: {} }, blocks, libs: [], workspaces: { main: { name: "Main", nodes, links } } };
  const file = path.join(dir, `bundle-${Date.now()}.json`);
  fs.writeFileSync(file, JSON.stringify(bundle));
  const child = spawn(process.execPath, [path.join(__dirname, "..", "circuit-runtime.js"), "--bundle", file, "--deps", dir, "--data", path.join(dir, "data"), "--debug", "--stop-on-input"], { stdio: ["pipe", "pipe", "pipe"] });
  const seen = [];
  let buf = "";
  let err = "";
  child.stdout.on("data", (d) => {
    buf += d;
    for (const line of buf.split("\n").slice(0, -1)) {
      if (line.startsWith("@@xc ")) {
        const e = JSON.parse(line.slice(5));
        if (e.t === "log") seen.push(`${e.level}: ${e.text}`);
      }
    }
    buf = buf.slice(buf.lastIndexOf("\n") + 1);
  });
  child.stderr.on("data", (d) => (err += d));
  const exited = new Promise((r) => child.on("exit", r));
  return (async () => {
    const started = Date.now();
    while (!done(seen) && Date.now() - started < 15000) await new Promise((r) => setTimeout(r, 50));
    child.stdin.write("stop\n");
    await exited;
    if (err.trim()) seen.push(`stderr: ${err.trim()}`);
    return seen;
  })();
}

const has = (seen, text) => seen.some((s) => s.endsWith(text));

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-timers-"));

  // First run: schedules, replaces by key, cancels by name, lists.
  const chain = ["start", "s1", "s2", "s3", "s3log", "s4", "s5", "c1", "c1log", "list", "listLog"];
  const nodes = {
    start: n("core.on_start"),
    s1: n("core.schedule", { name: "ping", when: "in", wait: "1s", data: { n: 1 } }),
    s2: n("core.schedule", { name: "ping", when: "in", wait: "1", data: { n: 2 }, key: "k" }),
    s3: n("core.schedule", { name: "ping", when: "in", wait: "1s", data: { n: 3 }, key: "k" }),
    s3log: say("replaced ${#s3.replaced}"),
    s4: n("core.schedule", { name: "later", when: "in", wait: "2s", data: "kept over a restart" }),
    s5: n("core.schedule", { name: "gone", when: "at", at: new Date(Date.now() + 3600000).toISOString() }),
    c1: n("core.cancel_scheduled", { by: "name", name: "gone" }),
    c1log: say("cancelled ${#c1.count}"),
    list: n("core.list_scheduled", { name: "" }),
    listLog: say("waiting ${#list.count}: ${#list.timers.map(t => t.name).join(',')}"),
    onPing: n("core.on_scheduled", { name: "ping" }),
    pingLog: say("ping ${data.n} on time ${late < 2000}"),
  };
  const links = [];
  for (let i = 0; i + 1 < chain.length; i++) links.push(`${chain[i]}.next > ${chain[i + 1]}.in`);
  links.push("onPing.next > pingLog.in");
  const first = await runBot(dir, nodes, links, (s) => has(s, "ping 1 on time true") && has(s, "ping 3 on time true"));
  for (const want of ["replaced true", "cancelled 1", "waiting 3: ping,ping,later", "ping 1 on time true", "ping 3 on time true"]) {
    assert.ok(has(first, want), `first run logged "${want}":\n${first.join("\n")}`);
  }
  assert.ok(!first.some((s) => s.includes("ping 2")), "the timer with the replaced key didn't run");
  // "later" had no listener in this run and wasn't due yet: it's still saved.
  const saved = JSON.parse(fs.readFileSync(path.join(dir, "data", "store.json"), "utf8"))["ns:xc.timers"];
  assert.deepStrictEqual(Object.values(saved).map((t) => t.name), ["later"]);

  // Second run, after "later" was due: it runs at start.
  await new Promise((r) => setTimeout(r, 2200));
  const second = await runBot(dir, { onLater: n("core.on_scheduled", { name: "later" }), log: say("later: ${data}") }, ["onLater.next > log.in"], (s) => has(s, "later: kept over a restart"));
  assert.ok(has(second, "later: kept over a restart"), `second run:\n${second.join("\n")}`);
  const after = JSON.parse(fs.readFileSync(path.join(dir, "data", "store.json"), "utf8"))["ns:xc.timers"] || {};
  assert.deepStrictEqual(after, {}, "it ran once and is gone");
  fs.rmSync(dir, { recursive: true, force: true });

  // Missed timers (an hour late): run where catching up is on, dropped where it's off.
  const store = new Store(fs.mkdtempSync(path.join(os.tmpdir(), "xc-timers-store-")), { log: null });
  const warned = [];
  const timers = new Timers({ store, log: { warn: (t) => warned.push(t), error: (t) => warned.push(t) } });
  const got = [];
  timers.on("missed", (t) => got.push(["catch up", t.data, t.late > 3000000]));
  timers.on("missed", (t) => got.push(["no catch up", t.data]), { catchUp: false });
  await timers.schedule({ name: "missed", due: Date.now() - 3600000, data: { user: new (class User { constructor() { this.id = "42"; this.client = {}; } })() } });
  await timers.schedule({ name: "nobody", due: Date.now() - 10, data: 1 });
  await timers.start();
  await new Promise((r) => setTimeout(r, 50));
  assert.deepStrictEqual(got, [["catch up", { user: { id: "42" } }, true]]);
  assert.ok(warned.some((w) => w.includes('"nobody"')), warned.join("\n"));
  assert.deepStrictEqual(await timers.list(), []);
  await assert.rejects(timers.schedule({ name: "", due: Date.now() }), /needs a name/);
  await assert.rejects(timers.cancel({}), /Say which/);
  store.flush = () => {};

  console.log("timers test passed");
  process.exit(0);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
