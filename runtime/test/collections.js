// Blocks never give Collections (discord.js's and Fluxer's Maps of things by
// id): a block's Collection output is a list of the items, from value and
// action blocks and events alike, and a Collection put where a list goes
// becomes its items too. Maps that aren't Collections stay as they are.
// Usage: node runtime/test/collections.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");
const assert = require("assert");
const { listsForCollections, coerce } = require("../circuit-runtime.js");

// Without the runtime running.
class Collection extends Map {}
const people = new Collection([["1", "ada"], ["2", "bo"]]);
assert.deepStrictEqual(listsForCollections({ people, n: 2 }), { people: ["ada", "bo"], n: 2 });
const plain = new Map([["a", 1]]);
assert.strictEqual(listsForCollections({ plain }).plain, plain, "a plain Map stays");
assert.deepStrictEqual(coerce(people, "list<text>"), ["ada", "bo"]);
assert.strictEqual(coerce(people, "any"), people, "only where a list goes");

// Collection to List, for Collections from JavaScript (${message.mentions.users}).
{
  const { ungroupBlock } = require("../circuit-runtime.js");
  const toList = ungroupBlock(require(path.join(__dirname, "..", "..", "plugins", "xc.core", "blocks", "lists", "to_list.js")));
  const run = (value, take = "values") => toList.run({ fields: { value, take } });
  assert.deepStrictEqual(run(people), { list: ["ada", "bo"], count: 2 });
  assert.deepStrictEqual(run(people, "keys").list, ["1", "2"]);
  assert.deepStrictEqual(run(people, "pairs").list, [{ key: "1", value: "ada" }, { key: "2", value: "bo" }]);
  assert.deepStrictEqual(run(new Set(["a", "b"])).list, ["a", "b"]);
  assert.deepStrictEqual(run({ x: 1, y: 2 }).list, [1, 2], "an object's values");
  assert.deepStrictEqual(run(["as", "is"]).list, ["as", "is"]);
  assert.deepStrictEqual(run("one").list, ["one"]);
  assert.deepStrictEqual(run(null), { list: [], count: 0 });
}

const blockSource = (obj) => `class Collection extends Map {}\nmodule.exports = ${obj};`;
const bundle = {
  project: { name: "Collections", mode: "test", settings: {} },
  types: {},
  blocks: {
    "t.start": {
      file: "start.js",
      source: blockSource(`{ id: "t.start", kind: "event", info: { name: "Start" }, listen(ctx, emit) { setTimeout(() => emit({ members: new Collection([["1", "ada"], ["2", "bo"]]) }), 50); } }`),
    },
    "t.roles": {
      file: "roles.js",
      source: blockSource(`{ id: "t.roles", kind: "value", info: { name: "Roles" }, outputs: { roles: { type: "list<text>" }, any: { type: "any" } },
        run() { return { roles: new Collection([["r1", "mods"], ["r2", "fans"]]), any: new Collection([["x", "cy"]]) }; } }`),
    },
    "t.fetch": {
      file: "fetch.js",
      source: blockSource(`{ id: "t.fetch", kind: "action", info: { name: "Fetch" }, outputs: { messages: { type: "list<text>" } },
        run() { return { messages: new Collection([["m1", "hi"], ["m2", "yo"]]) }; } }`),
    },
    "t.show": {
      file: "show.js",
      source: blockSource(`{ id: "t.show", kind: "action", info: { name: "Show" }, fields: { members: { type: "list<text>" }, roles: { type: "list<text>" }, any: { type: "any" }, messages: { type: "list<text>" } },
        run({ fields, ctx }) {
          for (const k of ["members", "roles", "any", "messages"]) ctx.log.info(k + " " + (Array.isArray(fields[k]) ? fields[k].join(",") : "not a list"));
        } }`),
    },
  },
  libs: [],
  workspaces: {
    main: {
      name: "Main",
      nodes: {
        s: { block: "t.start@1", at: [0, 0] },
        r: { block: "t.roles@1", at: [0, 0] },
        f: { block: "t.fetch@1", at: [0, 0] },
        show: { block: "t.show@1", at: [0, 0] },
      },
      links: ["s.next > f.in", "f.next > show.in", "s.members > show.members", "r.roles > show.roles", "r.any > show.any", "f.messages > show.messages"],
    },
  },
};

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-collections-"));
const bundlePath = path.join(dir, "bundle.json");
fs.writeFileSync(bundlePath, JSON.stringify(bundle));
const root = path.join(__dirname, "..", "..");
const child = spawn(process.execPath, [path.join(root, "runtime", "circuit-runtime.js"), "--bundle", bundlePath, "--deps", dir, "--data", path.join(dir, "data"), "--debug"]);
let out = "";
child.stdout.on("data", (d) => (out += d));
child.stderr.on("data", (d) => (out += d));

const logsOf = () =>
  out
    .split(/\r?\n/)
    .filter((l) => l.startsWith("@@xc "))
    .map((l) => JSON.parse(l.slice(5)))
    .filter((e) => e.t === "log")
    .map((e) => `${e.level}: ${e.text}`);
const expected = ["info: members ada,bo", "info: roles mods,fans", "info: any cy", "info: messages hi,yo"];
const started = Date.now();
const poll = setInterval(() => {
  const logs = logsOf();
  if (!expected.every((s) => logs.includes(s)) && Date.now() - started < 20000) return;
  clearInterval(poll);
  child.on("exit", () => fs.rmSync(dir, { recursive: true, force: true }));
  child.kill();
  assert.ok(!/"Collection"/.test(out), "the debugger shows lists, not Collections");
  for (const s of expected) assert.ok(logs.includes(s), `expected "${s}" in:\n${logs.join("\n")}\n${out.slice(-2000)}`);
  console.log("collections test passed");
}, 100);
