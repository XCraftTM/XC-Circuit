// The Fluxer blocks for roles, bans, emojis, webhooks, invites, threads,
// permissions, reactions, waiting, the audit log and the server, and the
// events for bans, roles, members, voice, invites and emojis: run against a
// stand-in for the fluxer.js client (the methods of @fluxerjs/core 3.3).
// Usage: node runtime/test/fluxer_more.js
"use strict";

const fs = require("fs");
const path = require("path");
const assert = require("assert");
const Module = require("module");
const { EventEmitter } = require("events");
const { ungroupBlock } = require("../circuit-runtime.js");
const lib = require("../../plugins/xc.fluxer/runtime/fluxer.js").helpers;

// Send with Webhook loads @fluxerjs/core for webhooks given by their key.
const fromToken = [];
const load = Module._load;
Module._load = function (request, ...rest) {
  if (request === "@fluxerjs/core") return { Webhook: { fromToken: (client, id, token) => (fromToken.push([id, token]), { send: async (o) => ({ via: id, ...o }) }) } };
  return load.call(this, request, ...rest);
};

const dir = path.join(__dirname, "..", "..", "plugins", "xc.fluxer", "blocks");
const all = {};
for (const group of fs.readdirSync(dir)) for (const f of fs.readdirSync(path.join(dir, group))) {
  const b = ungroupBlock(require(path.join(dir, group, f)));
  all[b.id] = b;
}

const calls = [];
const record = (what, value) => async (...args) => (calls.push([what, ...args]), value);
const client = new EventEmitter();
client.user = { id: "bot", leaveGuild: record("leave") };
client.users = { resolve: async (id) => ({ id, username: `u${id}`, bot: id === "bot" }) };
const role = (id, extra = {}) => ({ id, name: `r${id}`, color: 0x5865f2, position: Number(id), hoist: false, mentionable: true, guildId: "g1", edit: async function (o) {
    return { ...this, ...o, edited: true };
  }, delete: record(`delete role ${id}`), ...extra });
const emoji = { id: "e1", name: "wave", animated: false, url: () => "https://cdn/e1.png", delete: record("delete emoji") };
const server = {
  id: "g1",
  name: "Home",
  fetchRoles: async () => [role("1"), role("3"), role("2")],
  createRole: async (o) => role("9", o),
  fetchBans: async () => [{ user: { id: "5" }, reason: "spam", expiresAt: null }],
  fetchEmojis: async () => [emoji],
  createEmoji: async (o) => ({ id: "e2", ...o }),
  createSticker: async (o) => ({ id: "s1", ...o }),
  fetchStickers: async () => [],
  fetchWebhooks: async () => [{ id: "w1" }],
  fetchInvites: async () => [{ code: "abc", uses: 3, maxUses: 0, inviter: { id: "7" }, expiresAt: null, channelSnapshot: { id: "c1" } }],
  fetchInvite: async (code) => ({ code, uses: 4, inviter: { id: "7" }, delete: record(`delete invite ${code}`) }),
  fetchAuditLogs: async (o) => ({ entries: [{ id: "a1", action_type: 20, user_id: "7", target_id: "5", reason: "rude", changes: [] }], users: [], asked: o }),
  edit: async (o) => ({ ...server, ...o }),
  fetchChannels: async () => [{ id: "c2", name: "voice", type: 2, position: 2 }, { id: "c1", name: "general", type: 0, position: 1 }],
  members: { search: async (o) => ({ members: [{ userId: "5", username: "ada", globalName: null, nickname: null, member: { id: "5" } }], total: 12, asked: o }) },
};
client.guilds = new Map([["g1", server]]);
client.guilds.resolve = async (id) => client.guilds.get(id);
const thread = { id: "t1", name: "Help", setArchived: async (a) => ({ ...thread, archived: a }), setLocked: async (l) => ({ ...thread, locked: l }), edit: async (o) => ({ ...thread, ...o }), join: record("join"), leave: record("leave thread") };
const channel = {
  id: "c1",
  guildId: "g1",
  send: async () => ({}),
  createWebhook: async (o) => ({ id: "w9", token: "secret", ...o }),
  fetchWebhooks: async () => [{ id: "w2" }],
  threads: { create: async (o) => ({ ...thread, ...o }), createPost: async (o) => ({ ...thread, ...o, post: true }) },
  permissionOverwrites: { edit: record("overwrite"), delete: record("reset overwrite") },
};
client.channels = { resolve: async () => channel };
const fluxer = lib.makeApi(client, () => "!");
const defaults = (id) => Object.fromEntries(Object.entries(all[id].fields || {}).filter(([, f]) => "default" in f).map(([k, f]) => [k, f.default]));
const run = (id, fields, extra = {}) => all[id].run({ fields: { ...defaults(id), ...fields }, ctx: { fluxer, ...extra } });
const listen = (id, fields = {}) => {
  const seen = [];
  all[id].listen({ fields: { ...defaults(id), ...fields }, fluxer }, (o) => seen.push(o));
  return seen;
};

(async () => {
  // ---- Roles ----
  let o = await run("fluxer.manage_role", { action: "create", server, name: "Mods", color: "#ff0000", hoist: "on", permissions: "ManageMessages, KickMembers" });
  assert.deepStrictEqual([o.role.name, o.role.color, o.role.permissions, o.role.hoist, o.role.edited], ["Mods", 0xff0000, ["ManageMessages", "KickMembers"], true, true]);
  await assert.rejects(run("fluxer.manage_role", { action: "create", server }), /name/);
  await assert.rejects(run("fluxer.manage_role", { action: "edit", role: role("1"), color: "red" }), /isn't a color/);
  o = await run("fluxer.manage_role", { action: "delete", role: role("1"), reason: "old" });
  assert.deepStrictEqual(calls.pop(), ["delete role 1", "old"]);
  o = await run("fluxer.list_roles", { server });
  assert.deepStrictEqual(o.names, ["r3", "r2", "r1"], "highest first");
  assert.strictEqual(all["fluxer.role_info"].run({ fields: { role: role("4"), info: "color" } }).value, "#5865f2");
  assert.strictEqual(all["fluxer.role_info"].run({ fields: { role: role("4"), info: "mention" } }).value, "<@&4>");

  // ---- Bans, emojis ----
  o = await run("fluxer.bans", { action: "list", server });
  assert.deepStrictEqual([o.count, o.bans[0].reason], [1, "spam"]);
  o = await run("fluxer.bans", { action: "check", server, who: "<@5>" });
  assert.deepStrictEqual([o.banned, o.reason], [true, "spam"]);
  o = await run("fluxer.emojis", { action: "listEmojis", server });
  assert.deepStrictEqual(o.names, ["wave"]);
  o = await run("fluxer.emojis", { action: "createEmoji", server, name: "cat", image: "data:image/png;base64,AAAA" });
  assert.deepStrictEqual([o.item.image, o.text], ["data:image/png;base64,AAAA", "<:cat:e2>"]);
  await assert.rejects(run("fluxer.emojis", { action: "createEmoji", server, name: "cat", image: "not a link" }), /link/);
  await run("fluxer.emojis", { action: "deleteEmoji", server, name: "wave" });
  assert.strictEqual(calls.pop()[0], "delete emoji");
  o = await run("fluxer.emojis", { action: "createSticker", server, name: "hi", image: "data:x", tags: "a, b" });
  assert.deepStrictEqual(o.item.tags, ["a", "b"]);

  // ---- Webhooks ----
  o = await run("fluxer.webhooks", { action: "create", channel, name: "News" });
  assert.deepStrictEqual([o.webhook.name, o.key], ["News", "w9/secret"]);
  o = await run("fluxer.webhooks", { action: "list", where: "server", server });
  assert.deepStrictEqual(o.webhooks, [{ id: "w1" }]);
  o = await run("fluxer.webhook_send", { key: "https://api.fluxer.app/webhooks/123/tok-en", text: "hi", username: "Bot" });
  assert.deepStrictEqual(fromToken.pop(), ["123", "tok-en"]);
  assert.deepStrictEqual([o.message.content, o.message.username], ["hi", "Bot"]);
  await assert.rejects(run("fluxer.webhook_send", { key: "nope", text: "x" }), /key/);

  // ---- Invites, threads, permissions ----
  o = await run("fluxer.invites", { action: "list", server });
  assert.deepStrictEqual([o.invites[0].code, o.invites[0].uses], ["abc", 3]);
  o = await run("fluxer.invites", { action: "info", server, code: "https://fluxer.gg/xyz" });
  assert.deepStrictEqual([o.uses, o.invite.code], [4, "xyz"]);
  o = await run("fluxer.create_thread", { from: "forum", channel, name: "Q", text: "How?" });
  assert.deepStrictEqual([o.thread.post, o.thread.message], [true, { content: "How?" }]);
  o = await run("fluxer.create_thread", { from: "message", message: { startThread: async (x) => ({ ...thread, ...x }) }, name: "Talk" });
  assert.strictEqual(o.thread.name, "Talk");
  o = await run("fluxer.thread_action", { action: "lock", thread });
  assert.strictEqual(o.thread.locked, true);
  await run("fluxer.set_permissions", { channel, for: "everyone", deny: "SendMessages" });
  assert.deepStrictEqual(calls.pop(), ["overwrite", "g1", { type: 0, allow: undefined, deny: ["SendMessages"] }]);
  await run("fluxer.set_permissions", { channel, for: "member", who: "<@5>", mode: "reset" });
  assert.deepStrictEqual(calls.pop(), ["reset overwrite", "5"]);

  // ---- Reactions, waiting ----
  const msg = { id: "m1", fetchReactionUsers: async (e) => [{ id: "1", bot: false, e }, { id: "2", bot: true }] };
  o = await run("fluxer.get_reactions", { message: msg, emoji: "<:party:42>" });
  assert.deepStrictEqual([o.count, o.users[0].e], [1, "party:42"]);
  const flows = [];
  const next = async (flow, out) => flows.push([flow, out]);
  const waiting = run("fluxer.wait_for", { kind: "message", channel, contains: "yes", seconds: 5 }, { next });
  await new Promise((r) => setImmediate(r));
  client.emit("messageCreate", { channelId: "c1", author: { id: "8", bot: false }, content: "no" });
  client.emit("messageCreate", { channelId: "c1", author: { id: "8", bot: false }, content: "Yes please" });
  await waiting;
  assert.deepStrictEqual([flows[0][0], flows[0][1].content], ["received", "Yes please"]);
  assert.strictEqual(client.listenerCount("messageCreate"), 0, "it stops listening");
  await run("fluxer.wait_for", { kind: "reaction", message: msg, seconds: 0.01 }, { next });
  assert.strictEqual(flows[1][0], "timedOut");

  // ---- Audit log, server, members, channels, search ----
  o = await run("fluxer.audit_log", { server, by: "<@7>", limit: 5 });
  assert.deepStrictEqual([o.newest.action, o.newest.targetId, o.newest.reason], [20, "5", "rude"]);
  o = await run("fluxer.edit_server", { server, name: "New" });
  assert.strictEqual(o.server.name, "New");
  o = await run("fluxer.find_members", { server, name: "ad", bots: "people" });
  assert.deepStrictEqual([o.ids, o.count, o.members], [["5"], 12, [{ id: "5" }]]);
  o = await run("fluxer.list_channels", { server, kind: "0" });
  assert.deepStrictEqual(o.names, ["general"]);
  client.searchMessages = async () => ({ indexing: true });
  await assert.rejects(run("fluxer.search_messages", { server, text: "x" }), /indexing/);
  await run("fluxer.leave_server", { server: "123" });
  assert.deepStrictEqual(calls.pop(), ["leave", "123"]);

  // ---- Events ----
  const bans = listen("fluxer.on_ban", { when: "ban" });
  client.emit("guildBanAdd", { guildId: "g1", user: { id: "5" }, reason: "spam", expiresAt: null });
  assert.deepStrictEqual([bans[0].server, bans[0].reason], [server, "spam"]);
  const roles = listen("fluxer.on_role_event", { when: "delete" });
  client.emit("guildRoleDelete", null, "g1", "77");
  assert.deepStrictEqual([roles[0].roleId, roles[0].role], ["77", null]);
  const changes = listen("fluxer.on_member_update");
  const rolesOf = (ids) => ({ roles: { roleIds: ids }, nickname: "x", guild: server });
  client.emit("guildMemberUpdate", rolesOf(["1", "2"]), rolesOf(["2", "3"]));
  assert.deepStrictEqual([changes[0].rolesAdded, changes[0].rolesRemoved], [["3"], ["1"]]);
  const joins = listen("fluxer.on_voice_state", { when: "join" });
  const moves = listen("fluxer.on_voice_state", { when: "move" });
  const leaves = listen("fluxer.on_voice_state", { when: "leave" });
  client.emit("voiceStateUpdate", { user_id: "5", guild_id: "g1", channel_id: "v1" });
  client.emit("voiceStateUpdate", { user_id: "5", guild_id: "g1", channel_id: "v2" });
  client.emit("voiceStateUpdate", { user_id: "5", guild_id: "g1", channel_id: null });
  assert.deepStrictEqual([joins.length, moves.length, leaves.length], [1, 1, 1]);
  assert.deepStrictEqual([moves[0].beforeChannelId, moves[0].channelId, leaves[0].beforeChannelId], ["v1", "v2", "v2"]);
  const invites = listen("fluxer.on_invite_event", { when: "delete" });
  client.emit("inviteDelete", { code: "abc", channel_id: "c1", guild_id: "g1" });
  assert.deepStrictEqual([invites[0].code, invites[0].channelId], ["abc", "c1"]);
  const emojis = listen("fluxer.on_expression_event", { kind: "emojis" });
  client.emit("guildEmojisUpdate", { guild_id: "g1", emojis: [{ name: "a" }, { name: "b" }] });
  assert.deepStrictEqual(emojis[0].names, ["a", "b"]);

  // ---- A refused request names the permission ----
  const msgOf = lib.explainError({ statusCode: 403, message: "Missing Permissions" }, all["fluxer.manage_role"], {});
  assert.match(msgOf, /needs this: Manage Roles\./);
  assert.strictEqual(lib.explainError({ statusCode: 404 }, all["fluxer.manage_role"], {}), null);

  console.log("fluxer more test passed");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
