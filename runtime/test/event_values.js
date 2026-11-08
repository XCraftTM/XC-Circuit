// An unconnected interaction or server input takes the event's own, by the
// same name; users, channels and other inputs don't (an unconnected Ban
// mustn't hit whoever started the run). A wire or a typed value still wins.
// Usage: node runtime/test/event_values.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");
const assert = require("assert");
const { isContextType } = require("../circuit-runtime.js");

assert.ok(isContextType("discord.interaction") && isContextType("discord.guild?") && isContextType("fluxer.server"));
assert.ok(!isContextType("discord.user") && !isContextType("discord.channel") && !isContextType("discord.member") && !isContextType("text"));

const blocks = {
  "test.event": {
    source: `module.exports = { id: "test.event", kind: "event", info: { name: "Test Event", version: "1.0.0" },
      outputs: { server: { type: "discord.guild" }, interaction: { type: "discord.interaction" }, user: { type: "discord.user" } },
      listen(ctx, emit) { setTimeout(() => emit({ server: { name: "Home" }, interaction: { id: "i1" }, user: { id: "u1" } }), 20); } };`,
  },
  "test.uses": {
    source: `module.exports = { id: "test.uses", kind: "action", info: { name: "Uses", version: "1.0.0" },
      fields: {
        label: { type: "text", default: "" },
        server: { type: "discord.guild", pin: "only", required: true },
        interaction: { type: "discord.interaction", pin: "only" },
        user: { type: "discord.user", pin: "only" },
      },
      run({ fields: f, ctx }) { ctx.log.info(f.label + ": server=" + (f.server && f.server.name) + " interaction=" + (f.interaction && f.interaction.id) + " user=" + (f.user ? f.user.id : "none")); } };`,
  },
  "test.other_server": {
    source: `module.exports = { id: "test.other_server", kind: "value", info: { name: "Other", version: "1.0.0" },
      outputs: { server: { type: "discord.guild" } }, run() { return { server: { name: "Wired" } }; } };`,
  },
};
for (const b of Object.values(blocks)) b.file = "test.js";
const n = (block, values = {}) => ({ block: block + "@1", at: [0, 0], values });
// A runtime library that watches runs: told when each starts and ends.
const watcher = {
  file: "watch.js",
  source: `module.exports = { id: "watch", setup(rt) { rt.watchRuns((outputs) => { rt.log.info("run started with " + Object.keys(outputs).sort().join(",")); return () => rt.log.info("run ended"); }); } };`,
};
const bundle = {
  project: { name: "Event values", mode: "test", settings: {}, data: {} },
  blocks,
  libs: [watcher],
  workspaces: {
    main: {
      name: "Main",
      nodes: { ev: n("test.event"), a: n("test.uses", { label: "unwired" }), b: n("test.uses", { label: "wired" }), other: n("test.other_server") },
      links: ["ev.next > a.in", "a.next > b.in", "other.server > b.server"],
    },
  },
};

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-event-values-"));
fs.writeFileSync(path.join(dir, "bundle.json"), JSON.stringify(bundle));
const child = spawn(process.execPath, [path.join(__dirname, "..", "circuit-runtime.js"), "--bundle", path.join(dir, "bundle.json"), "--deps", dir, "--data", dir, "--debug"]);
let out = "";
child.stdout.on("data", (d) => (out += d));
child.stderr.on("data", (d) => (out += d));
const logs = () =>
  out
    .split(/\r?\n/)
    .filter((l) => l.startsWith("@@xc "))
    .map((l) => JSON.parse(l.slice(5)))
    .filter((e) => e.t === "log")
    .map((e) => e.text);
const started = Date.now();
const poll = setInterval(() => {
  const done = logs().includes("run ended");
  if (!done && Date.now() - started < 15000) return;
  clearInterval(poll);
  child.kill();
  try {
    const lines = logs();
    assert.ok(lines.includes("unwired: server=Home interaction=i1 user=none"), `the event's server and interaction, not its user:\n${out}`);
    assert.ok(lines.includes("wired: server=Wired interaction=i1 user=none"), `a wire wins:\n${out}`);
    const begun = lines.indexOf("run started with interaction,server,user");
    assert.ok(begun >= 0 && begun < lines.indexOf("unwired: server=Home interaction=i1 user=none"), `the watcher hears the start first:\n${out}`);
    assert.ok(lines.indexOf("run ended") > lines.indexOf("wired: server=Wired interaction=i1 user=none"), `and the end last:\n${out}`);
    console.log("event values test passed");
  } catch (e) {
    console.error(e.message);
    process.exitCode = 1;
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}, 100);
