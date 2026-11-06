// The file blocks in a running bot: write, add to, read (text, JSON, CSV,
// lines, bytes), list, copy, move, delete and download, all inside the
// bot's files folder; paths outside it are refused.
// Usage: node runtime/test/files.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const assert = require("assert");
const { spawn } = require("child_process");

const root = path.join(__dirname, "..", "..");
const blocks = {};
for (const sub of ["files", "events", "debug"]) {
  const folder = path.join(root, "plugins", "xc.core", "blocks", sub);
  for (const f of fs.readdirSync(folder)) blocks[require(path.join(folder, f)).id] = { source: fs.readFileSync(path.join(folder, f), "utf8"), file: path.join(folder, f) };
}
const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-files-"));
const answers = path.join(dir, "answers.json");
fs.writeFileSync(answers, JSON.stringify({ "https://cdn.example/pics/cat%20one.png": { body: "PNGDATA" }, "https://cdn.example/big": { body: "x".repeat(2 * 1024 * 1024) } }));

const n = (block, values = {}) => ({ block: block + "@1", at: [0, 0], values });
const say = (message) => n("core.log", { message, level: "info" });
const steps = [
  ["w1", n("core.write_file", { path: "notes/today.txt", content: "first" })],
  ["w2", n("core.write_file", { path: "notes/today.txt", content: "second", mode: "append" })],
  ["r1", n("core.read_file", { path: "notes/today.txt" })],
  ["l1", say("text ${JSON.stringify(#r1.content)} ${#r1.size}")],
  ["w3", n("core.write_file", { path: "data/user.json", content: { name: "Alex", xp: 5 } })],
  ["r2", n("core.read_file", { path: "data/user.json", as: "json" })],
  ["l2", say("json ${#r2.content.name} ${#r2.content.xp + 1}")],
  ["w4", n("core.write_file", { path: "table.csv", as: "csv", content: [{ name: "Alex", note: 'says "hi", twice' }] })],
  ["w5", n("core.write_file", { path: "table.csv", as: "csv", mode: "append", content: [{ name: "Bea", note: "line\nbreak" }] })],
  ["r3", n("core.read_file", { path: "table.csv", as: "csv" })],
  ["l3", say("csv ${#r3.content.length} ${#r3.content[0].note} | ${JSON.stringify(#r3.content[1].note)}")],
  ["r4", n("core.read_file", { path: "notes/today.txt", as: "lines" })],
  ["l4", say("lines ${#r4.content.join('+')}")],
  ["r5", n("core.read_file", { path: "nope.txt" })],
  ["l5", say("missing ${#r5.exists} ${#r5.content}")],
  ["c1", n("core.file_action", { action: "copy", path: "data/user.json", to: "backup/user.json" })],
  ["c2", n("core.file_action", { action: "move", path: "backup/user.json", to: "backup/old.json" })],
  ["c3", n("core.file_action", { action: "exists", path: "backup/user.json" })],
  ["l6", say("moved away ${!#c3.exists}")],
  ["d1", n("core.download_file", { from: "https://cdn.example/pics/cat%20one.png", path: "pics/" })],
  ["l7", say("downloaded ${#d1.name} ${#d1.size}")],
  ["ls", n("core.list_files", { folder: "", match: "*.json", deep: true })],
  ["l8", say("listed ${#ls.files.map(f => f.path).sort().join(',')}")],
  ["x1", n("core.file_action", { action: "delete", path: "notes" })],
  ["x2", n("core.file_action", { action: "exists", path: "notes/today.txt" })],
  ["l9", say("deleted ${!#x2.exists}")],
  ["bad", n("core.read_file", { path: "../secret.txt" })],
];
const nodes = { start: n("core.on_start") };
const links = [];
let prev = "start";
for (const [id, node] of steps) {
  nodes[id] = node;
  links.push(`${prev}.next > ${id}.in`);
  prev = id;
}
// Paths outside, and too big a download, in flows of their own.
nodes.start2 = n("core.on_start");
nodes.abs = n("core.write_file", { path: path.join(os.tmpdir(), "xc-outside.txt"), content: "no" });
nodes.start3 = n("core.on_start");
nodes.big = n("core.download_file", { from: "https://cdn.example/big", path: "big.bin", maxMb: 1 });
links.push("start2.next > abs.in", "start3.next > big.in");

const bundle = { project: { name: "Files", mode: "test", settings: {}, data: {} }, blocks, libs: [], workspaces: { main: { name: "Main", nodes, links } } };
fs.writeFileSync(path.join(dir, "bundle.json"), JSON.stringify(bundle));
const child = spawn(process.execPath, ["-r", path.join(__dirname, "fixtures", "mock-fetch.js"), path.join(__dirname, "..", "circuit-runtime.js"), "--bundle", path.join(dir, "bundle.json"), "--deps", dir, "--data", path.join(dir, "data"), "--debug", "--stop-on-input"], {
  stdio: ["pipe", "pipe", "pipe"],
  env: { ...process.env, XC_MOCK_FETCH: answers },
});
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

(async () => {
  const started = Date.now();
  const errors = () => seen.filter((s) => s.startsWith("error:"));
  while (!(seen.some((s) => s.includes("deleted")) && errors().length >= 3) && Date.now() - started < 15000) await new Promise((r) => setTimeout(r, 50));
  child.stdin.write("stop\n");
  await exited;
  const all = `${seen.join("\n")}\n${err}`;
  const want = [
    'text "first\\nsecond\\n" 13',
    "json Alex 6",
    'csv 2 says "hi", twice | "line\\nbreak"',
    "lines first+second",
    "missing false ",
    "moved away true",
    "downloaded cat one.png 7",
    "listed backup/old.json,data/user.json",
    "deleted true",
  ];
  for (const w of want) assert.ok(seen.some((s) => s.endsWith(w)), `logged "${w}":\n${all}`);
  assert.ok(errors().some((e) => e.includes('"../secret.txt" is outside')), all);
  assert.ok(errors().some((e) => e.includes("xc-outside.txt") && e.includes("is a path on the computer")), all);
  assert.ok(errors().some((e) => e.includes("bigger than 1 MB")), all);
  assert.ok(!fs.existsSync(path.join(os.tmpdir(), "xc-outside.txt")), "nothing was written outside");
  assert.ok(!fs.existsSync(path.join(dir, "data", "files", "big.bin")));
  assert.strictEqual(fs.readFileSync(path.join(dir, "data", "files", "pics", "cat one.png"), "utf8"), "PNGDATA");
  fs.rmSync(dir, { recursive: true, force: true });
  console.log("files test passed");
  process.exit(0);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
