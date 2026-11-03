// Runs the Fluxer starter's Ping and Help workspaces in the runtime, with a
// stand-in @fluxerjs/core that logs in at once and then receives !ping and
// !help: the runtime library, events, values and replies work together.
// Usage: node runtime/test/fluxer_run.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");
const assert = require("assert");

const root = path.join(__dirname, "..", "..");
const plugin = path.join(root, "plugins", "xc.fluxer");
const block = (rel) => ({ source: fs.readFileSync(path.join(plugin, "blocks", rel), "utf8"), file: path.join(plugin, "blocks", rel), plugin: "xc.fluxer" });
const starter = (name) => JSON.parse(fs.readFileSync(path.join(plugin, "templates", "basics", "workspaces", `${name}.json`), "utf8"));

const bundle = {
  project: { name: "Fluxer Run", mode: "fluxer-bot", settings: { prefix: "!" }, data: {}, tokenEnv: "FLUXER_BOT_TOKEN" },
  blocks: {
    "fluxer.on_text_command": block("events/on_text_command.js"),
    "fluxer.bot_info": block("bot/bot_info.js"),
    "fluxer.send_message": block("messages/send_message.js"),
    "fluxer.build_embed": block("messages/build_embed.js"),
  },
  libs: [{ source: fs.readFileSync(path.join(plugin, "runtime", "fluxer.js"), "utf8"), file: path.join(plugin, "runtime", "fluxer.js") }],
  workspaces: { ping: starter("ping"), help: starter("help") },
};

// The stand-in package: a client that "logs in" and then gets two commands.
const STAND_IN = `
const { EventEmitter } = require("events");
class Client extends EventEmitter {
  constructor() { super(); this.guilds = new Map(); this.ws = { ping: 37 }; this.uptime = 0; this.user = null; }
  async login(token) {
    if (token !== "secret-token") throw new Error("bad token " + token);
    this.user = { id: "1", username: "TestBot", bot: true };
    setTimeout(() => {
      this.emit("ready");
      for (const content of ["!ping", "!help", "!nothing"]) {
        this.emit("messageCreate", {
          content, author: { id: "2", bot: false }, member: null, guild: null, channel: {},
          reply: async (payload, options) => { console.log("REPLY " + JSON.stringify({ to: content, payload, options })); return {}; },
        });
      }
    }, 20);
    return this;
  }
}
module.exports = { Client };
`;

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-fluxer-"));
const pkg = path.join(dir, "node_modules", "@fluxerjs", "core");
fs.mkdirSync(pkg, { recursive: true });
fs.writeFileSync(path.join(pkg, "package.json"), JSON.stringify({ name: "@fluxerjs/core", version: "3.3.0", main: "index.js" }));
fs.writeFileSync(path.join(pkg, "index.js"), STAND_IN);
const bundlePath = path.join(dir, "bundle.json");
fs.writeFileSync(bundlePath, JSON.stringify(bundle));

// The token comes from FLUXER_BOT_TOKEN, as in an exported Fluxer bot.
const env = { ...process.env, FLUXER_BOT_TOKEN: "secret-token", DISCORD_TOKEN: "wrong" };
const child = spawn(process.execPath, [path.join(root, "runtime", "circuit-runtime.js"), "--bundle", bundlePath, "--deps", dir, "--data", path.join(dir, "data")], { env });
let out = "";
child.stdout.on("data", (d) => (out += d));
child.stderr.on("data", (d) => (out += d));

setTimeout(() => {
  child.kill();
  const replies = out
    .split(/\r?\n/)
    .filter((l) => l.startsWith("REPLY "))
    .map((l) => JSON.parse(l.slice(6)));
  try {
    assert.ok(/Logged in as TestBot/.test(out), "logged in:\n" + out);
    assert.strictEqual(replies.length, 2, "!ping and !help, not !nothing:\n" + out);
    const ping = replies.find((r) => r.to === "!ping");
    assert.deepStrictEqual(ping.payload, { content: "Pong! 37 ms" });
    assert.deepStrictEqual(ping.options, { ping: true });
    const help = replies.find((r) => r.to === "!help");
    assert.strictEqual(help.payload.content, undefined, "an embed without text");
    assert.strictEqual(help.payload.embeds[0].title, "Commands");
    assert.strictEqual(help.payload.embeds[0].color, 0x6c7bff);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
  console.log("fluxer run test passed");
}, 2500);
