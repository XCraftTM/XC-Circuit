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

  // Premium.
  let asked;
  const ent = { id: "e", skuId: "s", userId: "u", isActive: () => true, isTest: () => false };
  const papp = {
    fetchSKUs: async () => coll([{ id: "s", name: "Gold", type: 5, slug: "gold" }]),
    entitlements: { fetch: async (opts) => ((asked = opts), coll([ent])), consume: log("consume"), createTest: async (o) => ({ id: "t", ...o }), deleteTest: log("deleteTest") },
    subscriptions: { fetch: async () => coll([{ id: "sub", status: 0, skuIds: ["s"], currentPeriodEndTimestamp: 0 }]) },
  };
  o = await run("premium/premium.js", { action: "check", user: { id: "u" }, sku: "s" }, { application: papp });
  assert.deepStrictEqual([o.owns, asked.skus, asked.excludeEnded], [true, ["s"], true]);
  o = await run("premium/premium.js", { action: "skus" }, { application: papp });
  assert.deepStrictEqual(o.list, [{ id: "s", name: "Gold", type: "subscription", slug: "gold" }]);
  o = await run("premium/premium.js", { action: "subscriptions", user: { id: "u" }, sku: "s" }, { application: papp });
  assert.deepStrictEqual([o.owns, o.list[0].status], [true, "active"]);
  await assert.rejects(run("premium/premium.js", { action: "check", for: "server", sku: "s" }, { application: papp }), /Connect a server/);

  // Bans.
  let bulk;
  const bg = {
    bans: {
      fetch: async (o) => {
        if (!o) return coll([{ id: "1", user: { id: "1", username: "spammer" }, reason: "spam" }]);
        if (o.user === "1") return { reason: "spam" };
        throw Object.assign(new Error("Unknown Ban"), { code: 10026 });
      },
      bulkCreate: async (ids, opts) => ((bulk = { ids, opts }), { bannedUsers: ids.slice(0, 1), failedUsers: ids.slice(1) }),
    },
  };
  o = await run("members/bans.js", { action: "check", server: bg, userId: "1" });
  assert.deepStrictEqual([o.banned, o.reason], [true, "spam"]);
  o = await run("members/bans.js", { action: "check", server: bg, userId: "2" });
  assert.strictEqual(o.banned, false, "an unknown ban means not banned");
  o = await run("members/bans.js", { action: "bulk", server: bg, users: [{ id: "111111111111111111" }], ids: "222222222222222222 111111111111111111", deleteHours: 1 });
  assert.deepStrictEqual([bulk.ids, bulk.opts.deleteMessageSeconds, o.failedIds], [["111111111111111111", "222222222222222222"], 3600, ["222222222222222222"]]);

  // Prune.
  let pruned;
  o = await run("members/prune_members.js", { server: { members: { prune: async (opts) => ((pruned = opts), 4) } }, days: 99 });
  assert.deepStrictEqual([o.count, pruned.days, pruned.dry], [4, 30, true]);

  // Forward.
  o = await run("messages/forward_message.js", { message: { forward: async (c) => ({ to: c }) }, channel: { id: "c" } });
  assert.strictEqual(o.forwarded.to.id, "c");

  // Channel tools.
  const chans = coll([{ id: "2", name: "b", type: 0, rawPosition: 2 }, { id: "1", name: "a", type: 0, rawPosition: 1 }, { id: "3", name: "v", type: 2, rawPosition: 0 }]);
  o = await run("channels/channel_tools.js", { action: "list", server: { channels: { fetch: async () => chans } }, type: "text" });
  assert.deepStrictEqual(o.list.map((c) => c.name), ["a", "b"]);
  await assert.rejects(run("channels/channel_tools.js", { action: "sync", channel: { parentId: null } }), /isn't in a category/);

  // Forum tags.
  let setTags;
  const forum = { availableTags: [{ id: "t1", name: "Solved" }], setAvailableTags: async (t) => ((setTags = t), { availableTags: t }) };
  o = await run("channels/forum_tags.js", { action: "add", forum, tags: "Bug, solved", emoji: "🐛" });
  assert.deepStrictEqual([setTags.map((t) => t.name), setTags[1].emoji], [["Solved", "Bug"], { id: null, name: "🐛" }]);
  let applied;
  o = await run("channels/forum_tags.js", { action: "apply", post: { parent: forum, setAppliedTags: async (ids) => (applied = ids) }, tags: "solved" });
  assert.deepStrictEqual([applied, o.names], [["t1"], ["Solved"]]);
  await assert.rejects(run("channels/forum_tags.js", { action: "apply", post: { parent: forum, setAppliedTags: async () => {} }, tags: "nope" }), /no tag nope/);

  // Delete invite.
  o = await run("server/delete_invite.js", { code: "https://discord.gg/abc" }, { fetchInvite: async (c) => ({ code: c, uses: 3, delete: log("invite.delete") }) });
  assert.deepStrictEqual([o.code, o.uses], ["abc", 3]);

  // Create Channel: Position typed, connected as text, or left at the end.
  const Module = require("module");
  const load0 = Module._load;
  Module._load = function (request, ...rest) {
    return request === "discord.js" ? { ChannelType: { GuildText: 0, GuildVoice: 2, GuildCategory: 4 }, PermissionFlagsBits: { ViewChannel: 1024n } } : load0.call(this, request, ...rest);
  };
  try {
    const made = [];
    const server = { channels: { create: async (opts) => (made.push(opts), { id: "c1", ...opts }) } };
    for (const position of [3, "2", -1, "", null]) await run("channels/create_channel.js", { server, name: "tickets", position });
    assert.deepStrictEqual(made.map((m) => m.position), [3, 2, undefined, undefined, undefined]);
  } finally {
    Module._load = load0;
  }

  // Find Role, Find Channel, Find Member: Missing when it isn't there (an error with nothing on Missing).
  const finder = async (rel, fields, wired) => {
    const b = load(rel);
    const took = [];
    const ctx = { wired: (f) => f === "missing" && wired, next: async (flow, out) => took.push([flow, out]), discord: { client: { channels: { fetch: async (id) => (id === "900000000000000001" ? { id, name: "logs" } : Promise.reject(new Error("Unknown Channel"))) } } } };
    const defaults = Object.fromEntries(Object.entries(b.fields).map(([k, f]) => [k, f.default ?? null]));
    await b.run({ fields: { ...defaults, ...fields }, ctx });
    return took[0];
  };
  const roleList = [{ id: "700000000000000001", name: "Support" }];
  const srv = {
    name: "Test",
    roles: { cache: { find: (fn) => roleList.find(fn) }, fetch: async (id) => roleList.find((r) => r.id === id) ?? Promise.reject(new Error("Unknown Role")) },
    channels: { cache: { find: (fn) => [{ id: "c1", name: "general" }].find(fn) } },
    members: { fetch: async (id) => (id === "42" ? { id } : Promise.reject(new Error("Unknown Member"))), search: async () => new Map() },
  };
  assert.deepStrictEqual(await finder("roles/find_role.js", { server: srv, role: "support" }, true), ["next", { role: roleList[0] }]);
  assert.deepStrictEqual(await finder("roles/find_role.js", { server: srv, role: "<@&700000000000000001>" }, true), ["next", { role: roleList[0] }]);
  assert.deepStrictEqual(await finder("roles/find_role.js", { server: srv, role: "Admins" }, true), ["missing", { role: null }]);
  await assert.rejects(finder("roles/find_role.js", { server: srv, role: "Admins" }, false), /No role "Admins"/, "nothing on Missing: an error, as before");
  assert.deepStrictEqual((await finder("channels/find_channel.js", { channel: "<#900000000000000001>" }, true))[1].channel.name, "logs");
  assert.deepStrictEqual(await finder("channels/find_channel.js", { channel: "900000000000000009" }, true), ["missing", { channel: null }]);
  assert.strictEqual((await finder("channels/find_channel.js", { channel: "#general", server: srv }, true))[1].channel.id, "c1");
  await assert.rejects(finder("channels/find_channel.js", { channel: "nope", server: srv }, false), /No channel named "nope"/);
  assert.deepStrictEqual(await finder("members/find_member.js", { server: srv, by: "id", user: "<@!42>" }, true), ["next", { member: { id: "42" } }]);
  assert.deepStrictEqual(await finder("members/find_member.js", { server: srv, by: "id", user: "7" }, true), ["missing", { member: null }]);
  await assert.rejects(finder("members/find_member.js", { server: srv, by: "name", name: "zed" }, false), /Nobody named "zed"/);

  // A slow run's interaction is answered in time: a button quietly, a command with "thinking…".
  const { autoDefer } = require(path.join(blocks, "..", "runtime", "discord.js"));
  const fake = (kind) => {
    const i = { kind, deferred: false, replied: false, did: [] };
    i.isRepliable = () => kind !== "autocomplete";
    i.isAutocomplete = () => kind === "autocomplete";
    i.isMessageComponent = () => kind === "button";
    i.deferUpdate = async () => ((i.deferred = true), i.did.push("deferUpdate"));
    i.deferReply = async () => ((i.deferred = true), i.did.push("deferReply"));
    i.reply = async (p) => ((i.replied = true), i.did.push(["reply", p.content]));
    i.editReply = async (p) => i.did.push(["editReply", p.content]);
    i.followUp = async (p) => i.did.push(["followUp", p.content, (p.flags || []).includes("Ephemeral")]);
    i.deleteReply = async () => i.did.push("deleteReply");
    i.update = async (p) => i.did.push(["update", p.content]);
    return i;
  };
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const button = fake("button");
  autoDefer(button, 20);
  const command = fake("command");
  autoDefer(command, 20);
  const quick = fake("command");
  const stop = autoDefer(quick, 20);
  stop();
  const answered = fake("command");
  autoDefer(answered, 20);
  answered.replied = true;
  assert.strictEqual(autoDefer(fake("autocomplete"), 20), undefined, "autocomplete can't wait");
  await wait(60);
  assert.deepStrictEqual([button.did, button.xcAutoDeferred], [["deferUpdate"], "update"], "a button: quietly");
  assert.deepStrictEqual([command.did, command.xcAutoDeferred], [["deferReply"], "reply"], "a command: thinking…");
  assert.deepStrictEqual([quick.did, answered.did], [[], []], "a run that ended, or answered, is left alone");

  // Send Message after that: a new message after a button, filled in after "thinking…".
  const say = async (interaction, mode, isPrivate = false) => {
    const b = load("messages/send_message.js");
    const defaults = Object.fromEntries(Object.entries(b.fields).map(([k, f]) => [k, f.default ?? null]));
    await b.run({ fields: { ...defaults, mode, text: "hi", private: isPrivate, interaction }, ctx: {} });
    return interaction.did.slice(-1)[0];
  };
  assert.deepStrictEqual(await say(button, "reply", true), ["followUp", "hi", true], "after a quiet button answer: a new (private) message");
  assert.deepStrictEqual(await say(command, "reply"), ["editReply", "hi"], "after thinking…: it's filled in");
  assert.deepStrictEqual(await say(command, "reply", true), "deleteReply", "private after thinking…: sent privately, thinking… taken away");
  assert.deepStrictEqual(command.did.slice(-2, -1)[0], ["followUp", "hi", true]);
  assert.deepStrictEqual(await say(fake("command"), "reply"), ["reply", "hi"], "in time: a normal reply");
  const defer = fake("command");
  defer.deferred = true;
  await say(defer, "defer");
  assert.deepStrictEqual(defer.did, [], "Defer after an automatic one does nothing");

  // Mention: users, roles, channels, typed ids, lists, @everyone and @here.
  const mention = async (target, kind = "auto") => (await run("format/mention.js", { target, kind })).text;
  assert.strictEqual(await mention({ id: "1", username: "ada" }), "<@1>");
  assert.strictEqual(await mention({ id: "2", hexColor: "#fff" }), "<@&2>");
  assert.strictEqual(await mention("123", "role"), "<@&123>");
  assert.strictEqual(await mention("<@456>"), "<@456>", "a typed mention stays one");
  assert.strictEqual(await mention([{ id: "1" }, { id: "4" }]), "<@1> <@4>");
  assert.strictEqual(await mention("", "everyone"), "@everyone");
  await assert.rejects(mention(""), /needs a user/);

  // Button: an emoji before the text, or an emoji alone (discord.js's
  // ButtonBuilder stood in for by one that keeps what was set).
  const realLoad = Module._load;
  class ButtonBuilder {
    constructor() { this.data = {}; }
    setStyle(v) { this.data.style = v; return this; }
    setLabel(v) { this.data.label = v; return this; }
    setEmoji(v) { this.data.emoji = v; return this; }
    setCustomId(v) { this.data.custom_id = v; return this; }
    setURL(v) { this.data.url = v; return this; }
    setDisabled(v) { this.data.disabled = v; return this; }
  }
  Module._load = function (request, ...rest) {
    return request === "discord.js" ? { ButtonBuilder, ButtonStyle: { Primary: 1, Secondary: 2, Success: 3, Danger: 4, Link: 5, Premium: 6 } } : realLoad.call(this, request, ...rest);
  };
  const made = async (fields) => (await run("messages/button.js", fields)).button.data;
  assert.deepStrictEqual(await made({ label: "Close", emoji: "🔒" }), { style: 1, disabled: false, label: "Close", custom_id: "my-button", emoji: "🔒" });
  assert.ok(!("label" in (await made({ label: "", emoji: "<:tick:123>" }))), "an emoji alone: no text");
  assert.strictEqual((await made({ label: "" })).label, " ", "neither: a blank text, as before");
  Module._load = realLoad;

  console.log("discord actions test passed");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
