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

  // Stage.
  const stage = { topic: "Talk" };
  const stageChannel = {
    stageInstance: stage,
    guild: { stageInstances: { create: async (c, o) => ({ ...o }), edit: async (c, o) => ({ ...o }), delete: log("stage.delete"), fetch: async () => stage } },
  };
  o = await run("voice/stage.js", { action: "start", channel: stageChannel, topic: "Q&A" });
  assert.deepStrictEqual([o.topic, o.live], ["Q&A", true]);
  o = await run("voice/stage.js", { action: "end", channel: stageChannel });
  assert.strictEqual(o.live, false);
  let suppressed;
  o = await run("voice/stage.js", { action: "speaker", member: { voice: { channel: stageChannel, setSuppressed: async (v) => (suppressed = v) } } });
  assert.strictEqual(suppressed, false);

  // Welcome screen: channel lines become welcome channels.
  let edited;
  const screen = { enabled: true, description: "Hi", welcomeChannels: new Map([["1", { channelId: "1", description: "Rules", emoji: "📜" }]]) };
  const guild = { editWelcomeScreen: async (e) => ((edited = e), screen), fetchWelcomeScreen: async () => screen };
  o = await run("server/welcome_screen.js", { action: "set", server: guild, channels: "<#1> | Read the rules | 📜\n\n2 | Chat" });
  assert.deepStrictEqual(edited.welcomeChannels, [
    { channel: "1", description: "Read the rules", emoji: "📜" },
    { channel: "2", description: "Chat", emoji: undefined },
  ]);
  assert.deepStrictEqual(o.channels, [{ channelId: "1", description: "Rules", emoji: "📜" }]);

  // Onboarding: questions from JSON, broken JSON is reported.
  const onboarding = { enabled: true, mode: 1, defaultChannels: new Map(), prompts: new Map() };
  const og = { editOnboarding: async (e) => ((edited = e), onboarding), fetchOnboarding: async () => onboarding };
  o = await run("server/onboarding.js", { action: "set", server: og, mode: "advanced", prompts: '[{"title":"Games?","options":[{"title":"MC"}]}]' });
  assert.deepStrictEqual([edited.mode, edited.prompts[0].title, edited.prompts[0].inOnboarding, o.mode], [1, "Games?", true, "advanced"]);
  await assert.rejects(run("server/onboarding.js", { action: "set", server: og, prompts: "[{" }), /valid JSON/);

  // Templates.
  const template = { code: "abc", name: "T", url: "https://discord.new/abc", sync: async () => template, delete: log("template.delete") };
  const tg = { name: "S", fetchTemplates: async () => new Coll([["abc", template]]), createTemplate: async (n) => ({ ...template, name: n }) };
  o = await run("server/server_templates.js", { action: "list", server: tg });
  assert.deepStrictEqual(o.list.map((t) => t.url), ["https://discord.new/abc"]);
  o = await run("server/server_templates.js", { action: "sync", server: tg, code: "https://discord.new/abc" });
  assert.strictEqual(o.code, "abc");
  o = await run("server/server_templates.js", { action: "info", code: "abc" }, { fetchGuildTemplate: async (c) => ({ ...template, code: c }) });
  assert.strictEqual(o.url, "https://discord.new/abc");

  // Widget.
  const wg = { id: "7", fetchWidgetSettings: async () => ({ enabled: true, channel: null }), fetchWidget: async () => ({ presenceCount: 3, members: new Map(), instantInvite: "inv" }), setWidgetSettings: log("widget.set") };
  o = await run("server/server_widget.js", { action: "info", server: wg });
  assert.deepStrictEqual([o.online, o.invite, o.image], [3, "inv", "https://discord.com/api/guilds/7/widget.png?style=banner2"]);

  // Integrations.
  const twitch = { id: "1", name: "Twitch", type: "twitch", delete: log("integration.delete") };
  const ig = { fetchIntegrations: async () => coll([twitch, { id: "2", name: "MEE6", type: "discord", application: { bot: { username: "MEE6" } } }]) };
  o = await run("server/integrations.js", { action: "list", server: ig });
  assert.deepStrictEqual(o.list.map((i) => i.bot), ["", "MEE6"]);
  o = await run("server/integrations.js", { action: "remove", server: ig, which: "twitch" });
  assert.deepStrictEqual([o.count, calls.some((c) => c[0] === "integration.delete")], [1, true]);

  console.log("discord actions test passed");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
