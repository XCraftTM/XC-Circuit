// Runs Discord action blocks against stand-in discord.js objects and checks
// what they call and return.
// Usage: node runtime/test/discord_actions.js
"use strict";

const path = require("path");
const assert = require("assert");
const { ungroupBlock } = require("../circuit-runtime.js");

const blocks = path.join(__dirname, "..", "..", "plugins", "xc.discord", "blocks");
const load = (rel) => ungroupBlock(require(path.join(blocks, rel)));

/** A discord.js Collection stand-in: a Map with find. */
class Coll extends Map {
  find(fn) {
    for (const v of this.values()) if (fn(v)) return v;
  }
}
const coll = (items) => new Coll(items.map((x) => [x.id, x]));

/** Runs a block with `fields` (defaults filled in) and a client. */
async function run(rel, fields, client = {}, extra = {}) {
  const block = load(rel);
  const defaults = Object.fromEntries(Object.entries(block.fields).map(([k, f]) => [k, f.default ?? null]));
  const out = await block.run({ fields: { ...defaults, ...fields }, ctx: { discord: { client }, ...extra } });
  for (const key of Object.keys(block.outputs)) assert.ok(key in out, `${block.id}: output ${key}`);
  return out;
}

const calls = [];
const log = (name) => (...args) => {
  calls.push([name, ...args]);
  return args;
};

(async () => {
  // Webhooks.
  const hook = { id: "1", name: "H", url: "https://discord.com/api/webhooks/1/abc", delete: log("hook.delete"), edit: async (o) => ({ ...hook, ...o }) };
  const channel = { createWebhook: async (o) => ({ ...hook, ...o }), fetchWebhooks: async () => coll([hook]), send: async (p) => ({ sent: p }) };
  let o = await run("channels/webhooks.js", { action: "create", channel, name: "Logs" });
  assert.deepStrictEqual([o.webhook.name, o.url], ["Logs", hook.url]);
  o = await run("channels/webhooks.js", { action: "list", channel });
  assert.deepStrictEqual(o.list.map((w) => w.url), [hook.url]);
  o = await run("channels/webhooks.js", { action: "edit", url: hook.url, name: "New" }, { fetchWebhook: async (id, token) => (id === "1" && token === "abc" ? hook : null) });
  assert.strictEqual(o.webhook.name, "New");
  await run("channels/webhooks.js", { action: "delete", webhook: hook });
  assert.ok(calls.some((c) => c[0] === "hook.delete"));
  await assert.rejects(run("channels/webhooks.js", { action: "delete", url: "nope" }), /webhook/);

  // Stickers.
  const sticker = { id: "9", name: "Wave", url: "u", tags: "👋", delete: log("sticker.delete") };
  const server = { stickers: { fetch: async () => coll([sticker]), create: async (o) => ({ id: "10", ...o }) } };
  o = await run("messages/stickers.js", { action: "send", server, channel, sticker: "wave", text: "hi" });
  assert.deepStrictEqual(o.message.sent, { stickers: ["9"], content: "hi" });
  o = await run("messages/stickers.js", { action: "create", server, name: "Cat", file: "cat.png" });
  assert.deepStrictEqual([o.sticker.name, o.sticker.tags], ["Cat", "🙂"]);
  await assert.rejects(run("messages/stickers.js", { action: "delete", server, sticker: "nope" }), /No sticker named nope/);

  // Bot emojis.
  const emoji = { id: "5", name: "wave", toString: () => "<:wave:5>", delete: log("emoji.delete") };
  const app = { emojis: { fetch: async () => coll([emoji]), create: async (o) => ({ id: "6", name: o.name, toString: () => `<:${o.name}:6>` }) } };
  o = await run("channels/app_emojis.js", { action: "find", name: ":wave:" }, { application: app });
  assert.strictEqual(o.emoji, "<:wave:5>");
  o = await run("channels/app_emojis.js", { action: "create", name: "my emoji", image: "x.png" }, { application: app });
  assert.strictEqual(o.emoji, "<:my_emoji:6>");

  console.log("discord actions test passed");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
