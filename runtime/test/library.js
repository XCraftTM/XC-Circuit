// Runs the core block library through the runtime and checks the results.
// Usage: node runtime/test/library.js
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
    const id = /id:\s*"([^"]+)"/.exec(src)[1];
    blocks[id] = { source: src, file };
  }
}

const n = (block, values, extra = {}) => ({ block: block + "@1", at: [0, 0], values, ...extra });
const log = (message) => n("core.log", { message, level: "info" });

const bundle = {
  project: { name: "Library", mode: "test", settings: {}, data: {} },
  blocks,
  libs: [],
  workspaces: {
    main: {
      name: "Main",
      nodes: {
        start: n("core.on_start", {}),
        setv: n("core.set_variable", { name: "greeting", scope: "global" }),
        txt: n("core.text", { text: "hello world" }),
        getv: n("core.get_variable", { name: "greeting", scope: "global" }),
        upper: n("core.text_tools", { tool: "upper" }),
        l1: log("upper: ${result}"),
        list: n("core.make_list", { lines: "b\na\nc\na" }),
        uniq: n("core.list_tools", { tool: "unique" }),
        sort: n("core.list_tools", { tool: "sort" }),
        join: n("core.list_tools", { tool: "join", other: "-" }),
        l2: log("list: ${result}"),
        sw: n("core.switch", { cases: "small\nbig" }),
        big: log("switch went big"),
        other: log("switch went other"),
        rep: n("core.repeat", { times: 3 }),
        l3: log("round ${count}"),
        json: n("core.json", { direction: "parse", input: '{"a":{"b":7}}' }),
        prop: n("core.get_property", { path: "a.b" }),
        l4: log("prop: ${value}"),
        rnd: n("core.random_number", { min: 5, max: 5 }),
        l5: log("random: ${number}"),
        lg: n("core.logic", { op: "not" }),
        l6: log("not: ${result}"),
        // A value block in the flow runs there once: both logs get its number.
        rnd2: n("core.random_number", { min: 1, max: 1000000000 }),
        l7: log("first: ${number}"),
        l8: log("second: ${number}"),
      },
      links: [
        "start.next > setv.in",
        "txt.text > setv.value",
        "setv.next > l1.in",
        "getv.value > upper.text",
        "upper.result > l1.result",
        "l1.next > l2.in",
        "list.list > uniq.list",
        "uniq.result > sort.list",
        "sort.result > join.list",
        "join.result > l2.result",
        "l2.next > sw.in",
        "sw.big > big.in",
        "sw.other > other.in",
        "big.next > rep.in",
        "rep.each > l3.in",
        "rep.done > l4.in",
        "json.result > prop.object",
        "prop.value > l4.value",
        "l4.next > l5.in",
        "rnd.number > l5.number",
        "l5.next > l6.in",
        "upper.result > lg.a",
        "lg.result > l6.result",
        "l6.next > rnd2.in",
        "rnd2.next > l7.in",
        "l7.next > l8.in",
        "rnd2.number > l7.number",
        "rnd2.number > l8.number",
      ],
    },
  },
};
// Switch reads the text "BIG" (case is ignored by default).
bundle.workspaces.main.nodes.swv = n("core.text", { text: "BIG" });
bundle.workspaces.main.links.push("swv.text > sw.value");

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-lib-"));
fs.writeFileSync(path.join(dir, "bundle.json"), JSON.stringify(bundle));
const child = spawn(process.execPath, [path.join(root, "runtime", "circuit-runtime.js"), "--bundle", path.join(dir, "bundle.json"), "--deps", dir, "--data", dir, "--debug"]);
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
const expected = ["upper: HELLO WORLD", "list: a-b-c", "switch went big", "round 1", "round 3", "prop: 7", "random: 5", "not: false", "first: ", "second: "];
// Waits until every expected line is there (a slow machine takes longer),
// then a little more for errors that would follow; gives up after 20 s.
const started = Date.now();
const poll = setInterval(() => {
  const ready = expected.every((s) => logsOf().some((l) => l.includes(s)));
  if (!ready && Date.now() - started < 20000) return;
  clearInterval(poll);
  setTimeout(() => {
    child.kill();
    const logs = logsOf();
    console.log(logs.join("\n"));
    for (const s of expected) assert.ok(logs.some((l) => l.includes(s)), `expected "${s}"`);
    assert.ok(!logs.some((l) => l.startsWith("error")), "no errors");
    const number = (label) => logs.find((l) => l.includes(`${label}: `)).split(`${label}: `)[1];
    assert.strictEqual(number("first"), number("second"), "a value block that ran in the flow keeps its result");
    console.log("\nlibrary test passed");
  }, 300);
}, 100);
