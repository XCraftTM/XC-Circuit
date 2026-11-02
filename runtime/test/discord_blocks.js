// Checks the Discord blocks without Discord: every block file is well formed,
// and the blocks that only work with text and dates give the right results.
// Usage: node runtime/test/discord_blocks.js
"use strict";

const fs = require("fs");
const path = require("path");
const assert = require("assert");

const dir = path.join(__dirname, "..", "..", "plugins", "xc.discord", "blocks");
const all = {};
for (const group of fs.readdirSync(dir)) {
  for (const f of fs.readdirSync(path.join(dir, group))) {
    const block = require(path.join(dir, group, f));
    assert.ok(block.id && block.name && block.description, `${group}/${f}: id, name and description`);
    assert.ok(["event", "action", "value"].includes(block.kind), `${block.id}: kind`);
    assert.ok(!all[block.id], `${block.id}: unique id`);
    for (const [key, field] of Object.entries(block.fields || {})) assert.ok(field.type, `${block.id}.${key}: type`);
    if (block.kind === "event") assert.strictEqual(typeof block.listen, "function", `${block.id}: listen()`);
    else assert.strictEqual(typeof block.run, "function", `${block.id}: run()`);
    all[block.id] = block;
  }
}

const run = (id, fields) => all[id].run({ fields: { ...defaults(id), ...fields }, ctx: {} });
const defaults = (id) => Object.fromEntries(Object.entries(all[id].fields || {}).filter(([, f]) => "default" in f).map(([k, f]) => [k, f.default]));

// Mention: users, roles and channels.
assert.strictEqual(run("discord.mention", { target: { id: "1", username: "ada" } }).text, "<@1>");
assert.strictEqual(run("discord.mention", { target: { id: "2", hexColor: "#fff" } }).text, "<@&2>");
assert.strictEqual(run("discord.mention", { target: { id: "3", isTextBased: () => true } }).text, "<#3>");
assert.strictEqual(run("discord.mention", { target: "4", kind: "role" }).text, "<@&4>");

// Timestamp.
assert.strictEqual(run("discord.timestamp", { date: "2026-01-01T00:00:00Z", style: "R" }).text, "<t:1767225600:R>");
assert.throws(() => run("discord.timestamp", { date: "not a date" }), /isn't a date/);

// Discord Text.
const fmt = (style, text, extra = {}) => run("discord.discord_text", { style, text, ...extra }).text;
assert.strictEqual(fmt("bold", "hi"), "**hi**");
assert.strictEqual(fmt("codeblock", "x = 1", { language: "py" }), "```py\nx = 1\n```");
assert.strictEqual(fmt("quote", "a\nb"), "> a\n> b");
assert.strictEqual(fmt("subtext", "small"), "-# small");
assert.strictEqual(fmt("link", "site", { url: "https://x.y" }), "[site](https://x.y)");
assert.strictEqual(fmt("escape", "*not bold*"), "\\*not bold\\*");

// Slash commands: names with spaces are subcommands and groups.
const { toDiscordCommands } = require("../../plugins/xc.discord/runtime/discord.js");
const cmds = toDiscordCommands(
  [
    { name: "ping", description: "Pong", options: [], dm: true },
    { name: "ticket open", description: "Open", options: [{ name: "topic", type: "string", description: "What", autocomplete: true }, { name: "n", type: "integer", description: "N", required: true }], permission: "ManageMessages" },
    { name: "ticket close", description: "Close", options: [] },
    { name: "admin config set", description: "Set", options: [] },
  ],
  (name) => (name === "ManageMessages" ? "8192" : undefined),
);
const byName = Object.fromEntries(cmds.map((c) => [c.name, c]));
assert.deepStrictEqual(byName.ping.contexts, [0, 1], "works in DMs");
assert.deepStrictEqual(byName.ticket.options.map((o) => [o.type, o.name]), [[1, "open"], [1, "close"]]);
assert.deepStrictEqual(byName.ticket.options[0].options.map((o) => o.name), ["n", "topic"], "required options first");
assert.strictEqual(byName.ticket.options[0].options[1].autocomplete, true);
assert.strictEqual(byName.ticket.default_member_permissions, "8192");
assert.deepStrictEqual(byName.ticket.contexts, [0]);
assert.strictEqual(byName.admin.options[0].type, 2, "three words make a group");
assert.strictEqual(byName.admin.options[0].options[0].name, "set");

console.log(`${Object.keys(all).length} Discord blocks checked\ndiscord blocks test passed`);
