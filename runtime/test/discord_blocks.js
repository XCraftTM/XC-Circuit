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

console.log(`${Object.keys(all).length} Discord blocks checked\ndiscord blocks test passed`);
