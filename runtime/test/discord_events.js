// Runs Discord event blocks against a stand-in client: the discord.js event
// each one listens for, with objects shaped like discord.js's, gives the
// outputs the block promises.
// Usage: node runtime/test/discord_events.js
"use strict";

const path = require("path");
const assert = require("assert");
const { EventEmitter } = require("events");
const { ungroupBlock } = require("../circuit-runtime.js");

const dir = path.join(__dirname, "..", "..", "plugins", "xc.discord", "blocks", "events");
const STUBS = { "discord.js": { AuditLogEvent: { 20: "MemberKick", MemberKick: 20 } } };

/** Listens with `fields`, fires `event` with `args`, and returns what the block emitted. */
async function fire(file, fields, event, ...args) {
  const block = ungroupBlock(require(path.join(dir, file)));
  const client = new EventEmitter();
  client.users = { fetch: async (id) => ({ id, username: "fetched" }) };
  client.guilds = { cache: new Map() };
  const out = [];
  block.listen({ fields, discord: { client }, require: (n) => STUBS[n] }, (o) => out.push(o));
  assert.ok(client.listenerCount(event) > 0, `${block.id} listens for ${event}`);
  client.emit(event, ...args);
  await new Promise((r) => setImmediate(r));
  for (const o of out) for (const key of Object.keys(block.outputs)) assert.ok(key in o, `${block.id}: output ${key}`);
  return out;
}

const guild = { id: "g1", name: "Now" };
const named = (name, extra = {}) => ({ name, guild, ...extra });

(async () => {
  let [o] = await fire("on_thread_event.js", { kind: "updated" }, "threadUpdate", named("old"), named("new", { archived: true }));
  assert.deepStrictEqual([o.beforeName, o.thread.name, o.archived], ["old", "new", true]);
  [o] = await fire("on_thread_event.js", { kind: "members" }, "threadMembersUpdate", new Map([["u", { user: { id: "u" } }]]), new Map(), named("t"));
  assert.deepStrictEqual(o.joined, [{ id: "u" }]);

  [o] = await fire("on_expression_event.js", { kind: "emojiUpdate" }, "emojiUpdate", named("a"), named("b", { id: "1", toString: () => "<:b:1>", imageURL: () => "u" }));
  assert.deepStrictEqual([o.beforeName, o.name, o.text, o.url], ["a", "b", "<:b:1>", "u"]);

  [o] = await fire("on_typing.js", { ignoreBots: true }, "typingStart", { user: { bot: false }, channel: {}, guild });
  assert.strictEqual(o.server, guild);
  assert.deepStrictEqual(await fire("on_typing.js", { ignoreBots: true }, "typingStart", { user: { bot: true }, channel: {} }), [], "bots are skipped");

  [o] = await fire("on_reactions_cleared.js", { kind: "all" }, "messageReactionRemoveAll", { id: "m", channel: {}, guild }, new Map([["a", 1], ["b", 2]]));
  assert.strictEqual(o.count, 2);
  assert.strictEqual(o.server, guild);

  // Messages, buttons and threads say which server they're in (empty in DMs).
  [o] = await fire("on_message.js", {}, "messageCreate", { content: "hi", author: { bot: false }, channel: {}, guild });
  assert.strictEqual(o.server, guild);
  [o] = await fire("on_message.js", {}, "messageCreate", { content: "hi", author: { bot: false }, channel: {} });
  assert.strictEqual(o.server, null, "a DM has no server");
  [o] = await fire("on_button.js", { match: "equals", customId: "ok" }, "interactionCreate", { isButton: () => true, customId: "ok", user: {}, channel: {}, message: {}, guild });
  assert.strictEqual(o.server, guild);
  [o] = await fire("on_thread_create.js", {}, "threadCreate", { name: "t", guild }, true);
  assert.strictEqual(o.server, guild);

  [o] = await fire("on_audit_log.js", { action: "memberkick" }, "guildAuditLogEntryCreate", { action: 20, executorId: "x", targetId: "t", reason: "spam", changes: [{ key: "nick", old: "a" }] }, guild);
  assert.deepStrictEqual([o.action, o.executor.username, o.reason, o.changes[0].new], ["MemberKick", "fetched", "spam", null]);
  assert.deepStrictEqual(await fire("on_audit_log.js", { action: "MemberBanAdd" }, "guildAuditLogEntryCreate", { action: 20 }, guild), [], "other actions are skipped");

  [o] = await fire("on_server_update.js", { kind: "settings" }, "guildUpdate", { name: "Old", icon: "a" }, { name: "New", icon: "a" });
  assert.deepStrictEqual([o.beforeName, o.changed], ["Old", ["name"]]);

  [o] = await fire("on_user_update.js", {}, "userUpdate", { username: "a", globalName: "A", avatar: "1", displayAvatarURL: () => "old" }, { username: "b", globalName: "A", avatar: "2" });
  assert.deepStrictEqual([o.beforeName, o.changed, o.beforeAvatar], ["a", ["username", "avatar"], "old"]);

  [o] = await fire("on_stage_event.js", { kind: "updated" }, "stageInstanceUpdate", { topic: "a" }, { topic: "b", guild });
  assert.deepStrictEqual([o.beforeTopic, o.topic], ["a", "b"]);

  [o] = await fire("on_automod_rule_event.js", { kind: "deleted" }, "autoModerationRuleDelete", named("Spam", { enabled: true }));
  assert.deepStrictEqual([o.name, o.enabled], ["Spam", true]);

  [o] = await fire("on_pins_update.js", {}, "channelPinsUpdate", { guild }, new Date(0));
  assert.strictEqual(o.lastPin, "1970-01-01T00:00:00.000Z");

  [o] = await fire("on_soundboard_event.js", { kind: "played" }, "voiceChannelEffectSend", { userId: "u", soundId: "5", guild: { soundboardSounds: { cache: new Map([["5", { name: "Airhorn" }]]) } }, channel: {} });
  assert.deepStrictEqual([o.name, o.soundId, o.user.id], ["Airhorn", "5", "u"]);

  [o] = await fire("on_command_permissions.js", {}, "applicationCommandPermissionsUpdate", { id: "c", guildId: "g", permissions: [{ id: "r", type: 1, permission: false }] });
  assert.deepStrictEqual(o.permissions, [{ id: "r", type: "role", allowed: false }]);

  [o] = await fire("on_scheduled_event.js", { kind: "deleted" }, "guildScheduledEventDelete", named("Party"));
  assert.strictEqual(o.name, "Party");

  [o] = await fire("on_purchase.js", { kind: "entitlementCreate", sku: "" }, "entitlementCreate", { id: "e", skuId: "s", userId: "u", guildId: null, isTest: () => true });
  assert.deepStrictEqual([o.skuId, o.entitlementId, o.user.id, o.test], ["s", "e", "u", true]);
  assert.deepStrictEqual(await fire("on_purchase.js", { kind: "entitlementCreate", sku: "other" }, "entitlementCreate", { id: "e", skuId: "s" }), [], "other items are skipped");
  [o] = await fire("on_purchase.js", { kind: "subscriptionUpdate", sku: "" }, "subscriptionUpdate", {}, { userId: "u", skuIds: ["s"], entitlementIds: ["e"], status: 1 });
  assert.strictEqual(o.status, "ending");

  // Voice Changed: several changes ticked; Any change; older projects' single names.
  const member = { user: { bot: false } };
  const lounge = { id: "1503044503819260024", name: "Lounge" };
  const state = (channel, extra = {}) => ({ member, guild, channelId: channel?.id ?? null, channel: channel ?? null, ...extra });
  const join = [state(null), state(lounge)];
  const leave = [state(lounge), state(null)];
  const mute = [state(lounge), state(lounge, { mute: true })];
  const voice = async (change, [old, now]) => (await fire("on_voice_state.js", { change, ignoreBots: true }, "voiceStateUpdate", old, now)).length;
  assert.deepStrictEqual([await voice(["join", "leave"], join), await voice(["join", "leave"], leave), await voice(["join", "leave"], mute)], [1, 1, 0], "joins and leaves, not mutes");
  assert.deepStrictEqual([await voice(["any"], mute), await voice([], join)], [1, 1], "Any change, or none ticked");
  assert.deepStrictEqual([await voice("join", join), await voice("join", leave), await voice("any", mute), await voice("join, leave", leave)], [1, 0, 1, 1], "saved as a name");
  [o] = await fire("on_voice_state.js", { change: ["join"], ignoreBots: true }, "voiceStateUpdate", ...join);
  assert.strictEqual(o.after, lounge);

  console.log("discord events test passed");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
