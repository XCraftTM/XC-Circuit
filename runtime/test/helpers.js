// Runs the helper blocks (cooldowns, leaderboards, timers, loops, calls,
// formatting) through the runtime and checks what they log.
// Usage: node runtime/test/helpers.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");
const assert = require("assert");

const root = path.join(__dirname, "..", "..");
const core = path.join(root, "plugins", "xc.core", "blocks");
const blocks = {};
for (const dir of fs.readdirSync(core)) {
  for (const f of fs.readdirSync(path.join(core, dir))) {
    const file = path.join(core, dir, f);
    const src = fs.readFileSync(file, "utf8");
    blocks[/id:\s*"([^"]+)"/.exec(src)[1]] = { source: src, file };
  }
}

const n = (block, values = {}, extra = {}) => ({ block: block + "@1", at: [0, 0], values, ...extra });
const log = (message) => n("core.log", { message, level: "info" });

// Actions run one after another: start > a > b > … (value blocks are read by the logs).
const chain = ["start", "cd1", "cd2", "sx1", "sx2", "lbLog", "keysLog", "del", "delLog", "iset", "loop", "after", "call", "callLog", "fmt", "obj", "code", "codeLog", "up", "len", "useLog", "rndLog"];
const nodes = {
  start: n("core.on_start"),
  // Cooldown: the first use is ready, the second waits.
  cd1: n("core.cooldown", { name: "t", seconds: 60 }),
  cd2: n("core.cooldown", { name: "t", seconds: 60 }),
  lReady: log("cooldown ready"),
  lWait: log("cooldown waiting ${secondsLeft}"),
  // Stored values, ranked.
  sx1: n("core.set_stored", { scope: "user", target: "u1", key: "xp", value: 50 }),
  sx2: n("core.set_stored", { scope: "user", target: "u2", key: "xp", value: 120 }),
  lb: n("core.leaderboard", { scope: "user", key: "xp", top: 5, who: "u1" }),
  lbLog: log("board ${text} | rank ${rank} of ${total}"),
  keys: n("core.list_stored_keys", { scope: "user", target: "u1" }),
  keysLog: log("keys ${keys}"),
  del: n("core.delete_stored", { scope: "user", target: "u2", key: "xp" }),
  delLog: log("deleted ${existed}"),
  // Loop While: i counts 0, 1, 2 and stops at 3.
  iset: n("core.set_variable", { name: "i", scope: "global", value: 0 }),
  iget: n("core.get_variable", { name: "i", scope: "global", fallback: 0 }),
  cmp: n("core.compare", { op: "lt", b: 3 }),
  loop: n("core.loop_while", { max: 50 }),
  inc: n("core.set_variable", { name: "i", scope: "global" }),
  plus: n("core.math", { op: "add", b: 1 }),
  iget2: n("core.get_variable", { name: "i", scope: "global", fallback: 0 }),
  after: log("loop done ${count}"),
  // Call / When Called / Return.
  call: n("core.call", { name: "double", data: 21 }),
  callLog: log("called ${result}"),
  called: n("core.on_called", { name: "double" }),
  twice: n("core.math", { op: "multiply", b: 2 }),
  ret: n("core.return"),
  // Formatting.
  num: n("core.format_number", { number: 1234.5, style: "plain" }),
  ord: n("core.format_number", { number: 2, style: "ordinal" }),
  dur: n("core.duration", { direction: "parse", input: "1h 30m" }),
  rx: n("core.regex", { text: "order 42 and 7", pattern: "\\d+", mode: "all" }),
  fmt: log("num ${num} ${ord} dur ${ms} ${durText} rx ${rx}"),
  // Objects.
  mk: n("core.make_object", { keys: "a", a: 1 }),
  set: n("core.set_property", { path: "b.c", value: 2 }),
  obj: log("obj ${result.a} ${result.b.c}"),
  // Run Code and Use Method.
  code: n("core.run_code", { inputs: "a, b", outputs: "sum, product", code: "return { sum: inputs.a + inputs.b, product: inputs.a * inputs.b };", a: 2, b: 3 }),
  codeLog: log("code ${sum} ${product}"),
  word: n("core.text", { text: "hello" }),
  up: n("core.use_method", { mode: "call", path: "toUpperCase" }),
  len: n("core.use_method", { mode: "read", path: "length" }),
  useLog: log("use ${upper} ${length}"),
  // One value block read by two inputs gives both the same value.
  rnd: n("core.random_number", { min: 1, max: 1000000000, whole: true }),
  rndLog: log("random ${a} ${b}"),
  // Every: once at start, then each second.
  tick: n("core.every", { mode: "interval", every: 1, unit: "seconds", now: true }),
  tickLog: log("tick ${count}"),
};
const links = [];
for (let i = 0; i < chain.length - 1; i++) {
  const from = chain[i];
  const flow = from === "cd1" ? "ready" : from === "cd2" ? "waiting" : from === "loop" ? "done" : "next";
  links.push(`${from}.${flow} > ${chain[i + 1]}.in`);
}
links.push(
  "cd1.ready > lReady.in",
  "cd2.waiting > lWait.in",
  "lb.text > lbLog.text",
  "lb.rank > lbLog.rank",
  "lb.total > lbLog.total",
  "keys.keys > keysLog.keys",
  "del.existed > delLog.existed",
  // The loop reads i < 3 before each round, and adds 1 in it.
  "iget.value > cmp.a",
  "cmp.result > loop.condition",
  "loop.each > inc.in",
  "iget2.value > plus.a",
  "plus.result > inc.value",
  "loop.count > after.count",
  "called.data > twice.a",
  "called.next > ret.in",
  "twice.result > ret.value",
  "num.text > fmt.num",
  "ord.text > fmt.ord",
  "dur.ms > fmt.ms",
  "dur.text > fmt.durText",
  "rx.result > fmt.rx",
  "mk.object > set.object",
  "set.result > obj.result",
  "tick.next > tickLog.in",
  "code.sum > codeLog.sum",
  "code.product > codeLog.product",
  "word.text > up.object",
  "word.text > len.object",
  "up.result > useLog.upper",
  "len.result > useLog.length",
  "rnd.number > rndLog.a",
  "rnd.number > rndLog.b",
);

const bundle = { project: { name: "Helpers", mode: "test", settings: {}, data: {} }, blocks, libs: [], workspaces: { main: { name: "Main", nodes, links } } };

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-helpers-"));
fs.writeFileSync(path.join(dir, "bundle.json"), JSON.stringify(bundle));
const child = spawn(process.execPath, [path.join(root, "runtime", "circuit-runtime.js"), "--bundle", path.join(dir, "bundle.json"), "--deps", dir, "--data", dir, "--debug"]);
let out = "";
child.stdout.on("data", (d) => (out += d));
child.stderr.on("data", (d) => (out += d));
setTimeout(() => {
  child.kill();
  const logs = out
    .split(/\r?\n/)
    .filter((l) => l.startsWith("@@xc "))
    .map((l) => JSON.parse(l.slice(5)))
    .filter((e) => e.t === "log")
    .map((e) => `${e.level}: ${e.text}`);
  console.log(logs.join("\n"));
  const has = (s) => assert.ok(logs.some((l) => l.includes(s)), `expected "${s}"`);
  has("cooldown ready");
  has("cooldown waiting 60");
  has("board 1. <@u2> · 120\n2. <@u1> · 50 | rank 2 of 2");
  has("keys xp");
  has("deleted true");
  has("loop done 3");
  has("called 42");
  has("num 1,234.5 2nd dur 5400000 1 h 30 min rx 42,7");
  has("obj 1 2");
  has("code 5 6");
  has("use HELLO 5");
  has("tick 1");
  has("tick 2");
  const random = logs.map((l) => /random (\d+) (\d+)/.exec(l)).find(Boolean);
  assert.ok(random && random[1] === random[2], "both inputs got the same random number");
  assert.ok(!logs.some((l) => l.startsWith("error")), "no errors");
  console.log("\nhelpers test passed");
}, 2600);
