// Pin type conversions: a value wired into an input of another type is
// turned into it by the type's `via` (a Member into a User input gets the
// member's user), alone or in a list. Values that already are the right type,
// or wired into `any`, stay as they are.
// Usage: node runtime/test/conversions.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");
const assert = require("assert");

const root = path.join(__dirname, "..", "..");

const blockSource = (obj) => `module.exports = ${obj};`;
const bundle = {
  project: { name: "Conversions", mode: "test", settings: {} },
  types: {
    "t.member": { check: "v => v?.kind === 'member'", converts: [{ to: "t.user", via: "v => v.user" }] },
    "t.user": { check: "v => v?.kind === 'user'" },
  },
  blocks: {
    "t.start": {
      file: "start.js",
      source: blockSource(`{ id: "t.start", kind: "event", info: { name: "Start" }, listen(ctx, emit) { setTimeout(() => emit({}), 50); } }`),
    },
    "t.member": {
      file: "member.js",
      source: blockSource(`{ id: "t.member", kind: "value", info: { name: "Member" }, outputs: { member: { type: "t.member" }, members: { type: "list<t.member>" }, user: { type: "t.user" } },
        run() { const m = (n) => ({ kind: "member", nick: n + "!", user: { kind: "user", name: n } }); return { member: m("alex"), members: [m("bo"), m("cy")], user: { kind: "user", name: "dee" } }; } }`),
    },
    "t.show": {
      file: "show.js",
      source: blockSource(`{ id: "t.show", kind: "action", info: { name: "Show" }, fields: { one: { type: "t.user" }, many: { type: "list<t.user>" }, raw: { type: "any" }, again: { type: "t.user" } },
        run({ fields, ctx }) {
          ctx.log.info("one " + fields.one?.kind + " " + fields.one?.name);
          ctx.log.info("many " + (fields.many || []).map((u) => u.kind + " " + u.name).join(", "));
          ctx.log.info("raw " + fields.raw?.kind);
          ctx.log.info("again " + fields.again?.kind + " " + fields.again?.name);
        } }`),
    },
  },
  libs: [],
  workspaces: {
    main: {
      name: "Main",
      nodes: {
        s: { block: "t.start@1", at: [0, 0] },
        m: { block: "t.member@1", at: [0, 0] },
        show: { block: "t.show@1", at: [0, 0] },
      },
      links: ["s.next > show.in", "m.member > show.one", "m.members > show.many", "m.member > show.raw", "m.user > show.again"],
    },
  },
};

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-conversions-"));
const bundlePath = path.join(dir, "bundle.json");
fs.writeFileSync(bundlePath, JSON.stringify(bundle));
const child = spawn(process.execPath, [path.join(root, "runtime", "circuit-runtime.js"), "--bundle", bundlePath, "--deps", dir, "--data", path.join(dir, "data"), "--debug"]);
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
  const has = (s) => assert.ok(logs.includes(s), `expected "${s}" in:\n${logs.join("\n")}`);
  has("info: one user alex");
  has("info: many user bo, user cy");
  has("info: raw member");
  has("info: again user dee");
  fs.rmSync(dir, { recursive: true, force: true });
  console.log("conversions test passed");
}, 1500);
