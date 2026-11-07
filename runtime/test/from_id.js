// Values found by their id: a pin type with `fromId` (a channel) can be given
// an id instead, typed into the block or as a text wired in, also as a
// mention (<#id>); the runtime looks it up before the block runs. Types that
// extend it (a thread) look it up the same way, and an unknown id stops the
// block with a clear message. Inputs a block adds in define() (Send
// Message's channel) take an id as well.
// Usage: node runtime/test/from_id.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");
const assert = require("assert");

const root = path.join(__dirname, "..", "..");

const blockSource = (obj) => `module.exports = ${obj};`;
const bundle = {
  project: { name: "From id", mode: "test", settings: {} },
  types: {
    "t.channel": { check: "v => v?.kind === 'channel'", converts: [], fromId: "(id, ctx) => (id === '42' || id === '7' ? { kind: 'channel', id, where: ctx.id } : null)" },
    "t.thread": { check: "v => v?.kind === 'channel'", converts: [], extends: "t.channel" },
  },
  blocks: {
    "t.start": {
      file: "start.js",
      source: blockSource(`{ id: "t.start", kind: "event", info: { name: "Start" }, listen(ctx, emit) { setTimeout(() => emit({}), 50); } }`),
    },
    "t.text": {
      file: "text.js",
      source: blockSource(`{ id: "t.text", kind: "value", info: { name: "Text" }, outputs: { text: { type: "text" } }, run() { return { text: "<#7>" }; } }`),
    },
    "t.show": {
      file: "show.js",
      source: blockSource(`{ id: "t.show", kind: "action", info: { name: "Show" }, fields: { channel: { type: "t.channel" }, thread: { type: "t.thread?" }, wired: { type: "t.channel" }, none: { type: "t.channel?" } },
        run({ fields, ctx }) {
          ctx.log.info("channel " + fields.channel?.kind + " " + fields.channel?.id + " " + fields.channel?.where);
          ctx.log.info("thread " + fields.thread?.kind + " " + fields.thread?.id);
          ctx.log.info("wired " + fields.wired?.kind + " " + fields.wired?.id);
          ctx.log.info("none " + JSON.stringify(fields.none ?? null));
        } }`),
    },
    "t.added": {
      file: "added.js",
      source: blockSource(`{ id: "t.added", kind: "action", info: { name: "Added" }, fields: { mode: { type: "select", default: "reply" } },
        define({ values }) { return values.mode === "send" ? { add: { channel: { type: "t.channel", pin: "only" } } } : {}; },
        run({ fields, ctx }) { ctx.log.info("added " + fields.channel?.kind + " " + fields.channel?.id); } }`),
    },
    "t.bad": {
      file: "bad.js",
      source: blockSource(`{ id: "t.bad", kind: "action", info: { name: "Bad" }, fields: { channel: { type: "t.channel", label: "Channel" } },
        run({ fields, ctx }) { ctx.log.info("bad ran " + fields.channel); } }`),
    },
  },
  libs: [],
  workspaces: {
    main: {
      name: "Main",
      nodes: {
        s: { block: "t.start@1", at: [0, 0] },
        txt: { block: "t.text@1", at: [0, 0] },
        show: { block: "t.show@1", at: [0, 0], values: { channel: " 42 ", thread: 42, none: "" } },
        added: { block: "t.added@1", at: [0, 0], values: { mode: "send", channel: "<#42>" } },
        bad: { block: "t.bad@1", at: [0, 0], values: { channel: "999" } },
      },
      links: ["s.next > show.in", "show.next > added.in", "added.next > bad.in", "txt.text > show.wired"],
    },
  },
};

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-from-id-"));
const bundlePath = path.join(dir, "bundle.json");
fs.writeFileSync(bundlePath, JSON.stringify(bundle));
const child = spawn(process.execPath, [path.join(root, "runtime", "circuit-runtime.js"), "--bundle", bundlePath, "--deps", dir, "--data", path.join(dir, "data"), "--debug"]);
let out = "";
child.stdout.on("data", (d) => (out += d));
child.stderr.on("data", (d) => (out += d));

const events = () =>
  out
    .split(/\r?\n/)
    .filter((l) => l.startsWith("@@xc "))
    .map((l) => JSON.parse(l.slice(5)));
const logs = () => events().filter((e) => e.t === "log").map((e) => `${e.level}: ${e.text}`);
const expected = ["info: channel channel 42 main:show", "info: thread channel 42", "info: wired channel 7", "info: none null", "info: added channel 42"];
const failed = () => JSON.stringify(events().filter((e) => e.t !== "log" || e.level === "error"));
// Waits until every expected line is there (a slow machine takes longer); gives up after 20 s.
const started = Date.now();
const poll = setInterval(() => {
  const seen = logs();
  const done = expected.every((s) => seen.includes(s)) && failed().includes('no channel with the id \\"999\\"');
  if (!done && Date.now() - started < 20000) return;
  clearInterval(poll);
  child.on("exit", () => fs.rmSync(dir, { recursive: true, force: true }));
  child.kill();
  for (const s of expected) assert.ok(seen.includes(s), `expected "${s}" in:\n${seen.join("\n")}\n${out}`);
  assert.ok(failed().includes('Channel: no channel with the id \\"999\\"'), `expected the unknown id to stop Bad:\n${out}`);
  assert.ok(!seen.some((s) => s.startsWith("info: bad ran")), "Bad must not run with an unknown id");
  console.log("from id test passed");
}, 100);
