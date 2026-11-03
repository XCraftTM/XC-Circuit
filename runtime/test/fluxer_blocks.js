// Checks the Fluxer blocks without Fluxer: every block file is well formed, and
// the blocks do what they say against a stand-in for the fluxer.js client
// (the methods and events of @fluxerjs/core 3.3).
// Usage: node runtime/test/fluxer_blocks.js
"use strict";

const fs = require("fs");
const path = require("path");
const assert = require("assert");
const { EventEmitter } = require("events");
const { ungroupBlock } = require("../circuit-runtime.js");
const library = require("../../plugins/xc.fluxer/runtime/fluxer.js");
const lib = library.helpers;
assert.strictEqual(library.id, "fluxer", "blocks reach the library as ctx.fluxer");

const dir = path.join(__dirname, "..", "..", "plugins", "xc.fluxer", "blocks");
const all = {};
for (const group of fs.readdirSync(dir)) {
  for (const f of fs.readdirSync(path.join(dir, group))) {
    const block = ungroupBlock(require(path.join(dir, group, f)));
    assert.ok(block.id.startsWith("fluxer.") && block.name && block.description, `${group}/${f}: id, name and description`);
    assert.ok(["event", "action", "value"].includes(block.kind), `${block.id}: kind`);
    assert.ok(!all[block.id], `${block.id}: unique id`);
    for (const [key, field] of Object.entries(block.fields || {})) assert.ok(field.type, `${block.id}.${key}: type`);
    if (block.kind === "event") assert.strictEqual(typeof block.listen, "function", `${block.id}: listen()`);
    else assert.strictEqual(typeof block.run, "function", `${block.id}: run()`);
    all[block.id] = block;
  }
}

// ---- Helpers of the runtime library ----------------------------------------

assert.deepStrictEqual(lib.parseCommand("!ban <@1> spam  now", "!"), { command: "ban", args: ["<@1>", "spam", "now"], rest: "<@1> spam  now" });
assert.deepStrictEqual(lib.parseCommand("?PING", "?"), { command: "ping", args: [], rest: "" });
assert.strictEqual(lib.parseCommand("hello !ping", "!"), null);
assert.strictEqual(lib.parseCommand("!", "!"), null);
assert.strictEqual(lib.id("<@!12>", "user"), "12");
assert.strictEqual(lib.id("<@&34>", "role"), "34");
assert.strictEqual(lib.id("<#56>", "channel"), "56");
assert.strictEqual(lib.id("<@&34>", "user"), null, "a role mention is no user");
assert.strictEqual(lib.id(" 78 ", "user"), "78");
assert.strictEqual(lib.id({ id: 9 }), "9");
assert.strictEqual(lib.id("someone", "user"), null);
{
  const e = lib.embed(
    { color: "#00D8F0", title: "Hi {who}", description: "", fields: [{ name: "A", value: "1", inline: true }, { name: "", value: "" }], footer: { text: "f" }, image: "https://x/i.png", time: "0" },
    (s) => s.replace("{who}", "Ada"),
  );
  assert.deepStrictEqual(e, {
    description: null,
    title: "Hi Ada",
    color: 0x00d8f0,
    fields: [{ name: "A", value: "1", inline: true }],
    footer: { text: "f", icon_url: undefined },
    image: { url: "https://x/i.png" },
    timestamp: "1970-01-01T00:00:00.000Z",
  });
}

// ---- A stand-in for the fluxer.js client -------------------------------------

const calls = [];
const record = (what) => (...args) => {
  calls.push([what, ...args]);
  return Promise.resolve();
};
const user = (id, extra = {}) => ({ id, username: `user${id}`, globalName: null, bot: false, displayAvatarURL: () => `https://cdn/${id}.png`, send: async (p) => ({ sentTo: id, ...p }), ...extra });
const channel = (id, name = `chan${id}`) => ({
  id,
  name,
  isTextBased: () => true,
  send: async (p) => ({ id: "sent", channelId: id, payload: p, delete: record("delete sent") }),
  messages: {
    fetch: async (q) => (typeof q === "string" ? { id: q } : new Map([["m2", { id: "m2" }], ["m1", { id: "m1" }]])),
  },
  bulkDelete: async (n) => Array.from({ length: n }, (_, i) => String(i)),
  createInvite: async (o) => ({ url: `https://fluxer.gg/abc?age=${o.maxAge}&uses=${o.maxUses}`, code: "abc" }),
  edit: async (o) => ({ ...channel(id, o.name ?? name), edited: o }),
  setSlowmode: async (s) => ({ ...channel(id, name), slowmode: s }),
  delete: record(`delete channel ${id}`),
});
const server = {
  id: "g1",
  name: "Guild",
  memberCount: 3,
  ownerId: "1",
  iconURL: () => "https://cdn/g1.png",
  createdAt: new Date(0),
  systemChannelId: "c1",
  roles: new Map([["r1", { id: "r1", guildId: "g1", name: "Mod", color: 0xff8800, position: 3 }]]),
  channels: new Map([["c1", channel("c1", "general")]]),
  fetchRoles: async () => [],
  fetchChannels: async () => [],
  fetchMember: async (id) => {
    if (id === "404") throw new Error("Unknown member");
    return member(id);
  },
  ban: record("ban"),
  unban: record("unban"),
  createChannel: async (o) => ({ ...channel("new", o.name), created: o }),
};
function member(id, roles = []) {
  const ids = new Set(roles);
  return {
    id,
    user: user(id),
    guild: server,
    nick: null,
    displayName: `Member ${id}`,
    joinedAt: new Date(1000),
    communicationDisabledUntil: null,
    roles: { roleIds: [...ids], has: (r) => ids.has(r), add: async (r) => ids.add(r), remove: async (r) => ids.delete(r) },
    permissions: { has: (p) => p === "BanMembers" },
    displayAvatarURL: () => `https://cdn/m${id}.png`,
    kick: record(`kick ${id}`),
    timeout: record(`timeout ${id}`),
    edit: record(`edit ${id}`),
  };
}

const client = new EventEmitter();
Object.assign(client, {
  user: { ...user("bot", { bot: true }), setPresence: (p) => calls.push(["presence", p]) },
  guilds: new Map([["g1", server]]),
  channels: { resolve: async (id) => (id === "900" ? channel("900") : Promise.reject(new Error("Unknown channel"))) },
  users: { resolve: async (id) => user(id) },
  ws: { ping: 42.4 },
  uptime: 5_400,
});
const fluxer = lib.makeApi(client, () => "!");

const defaults = (id) => Object.fromEntries(Object.entries(all[id].fields || {}).filter(([, f]) => "default" in f).map(([k, f]) => [k, f.default]));
const fill = (s, f) => String(s).replace(/\{(\w+)\}/g, (w, k) => (k in f ? String(f[k]) : w));
const run = (id, fields) => all[id].run({ fields: { ...defaults(id), ...fields }, ctx: { fluxer, fill } });
const listen = (id, fields = {}) => {
  const seen = [];
  all[id].listen({ fields: { ...defaults(id), ...fields }, fluxer }, (out) => seen.push(out));
  return seen;
};
const message = (content, extra = {}) => ({
  id: "m1",
  content,
  author: user("7"),
  member: member("7"),
  channel: channel("c1"),
  guild: server,
  channelId: "c1",
  guildId: "g1",
  createdAt: new Date(2000),
  pinned: false,
  mentions: [],
  referencedMessage: null,
  reply: async (p, o) => ({ replied: p, options: o }),
  edit: async (p) => ({ edited: p }),
  delete: record("delete m1"),
  react: record("react"),
  removeReaction: record("unreact"),
  removeAllReactions: record("clear reactions"),
  pin: record("pin"),
  unpin: record("unpin"),
  ...extra,
});

(async () => {
  // ---- Events ----
  const commands = listen("fluxer.on_text_command", { command: "ban, b" });
  client.emit("messageCreate", message("!b <@2> spam"));
  client.emit("messageCreate", message("!ping"));
  client.emit("messageCreate", message("?ban x"));
  client.emit("messageCreate", message("!ban x", { author: user("8", { bot: true }) }));
  assert.strictEqual(commands.length, 1, "aliases, the prefix and bots");
  assert.deepStrictEqual(commands[0].args, ["<@2>", "spam"]);
  assert.strictEqual(commands[0].rest, "<@2> spam");
  assert.deepStrictEqual([commands[0].first, commands[0].more], ["<@2>", "spam"]);
  assert.strictEqual(commands[0].server, server);

  const said = listen("fluxer.on_message", { contains: "hello" });
  client.emit("messageCreate", message("hello there"));
  client.emit("messageCreate", message("bye"));
  assert.deepStrictEqual(said.map((s) => s.content), ["hello there"]);

  const joins = listen("fluxer.on_member", { when: "join" });
  const leaves = listen("fluxer.on_member", { when: "leave" });
  client.emit("guildMemberAdd", member("5"));
  client.emit("guildMemberRemove", member("6"));
  assert.deepStrictEqual([joins.map((j) => j.user.id), leaves.map((l) => l.user.id)], [["5"], ["6"]]);

  const reactions = listen("fluxer.on_reaction", { emoji: "👍" });
  const custom = listen("fluxer.on_reaction", { emoji: "party" });
  client.emit("messageReactionAdd", { emoji: { name: "👍" }, userId: "3", messageId: "m1", channelId: "c1", message: null, user: null, member: null, reaction: { fetchMessage: async () => ({ id: "m1" }) } });
  client.emit("messageReactionAdd", { emoji: { name: "party", id: "99" }, userId: "3", messageId: "m1", channelId: "c1", message: { id: "m1" }, user: user("3"), member: null });
  await new Promise((r) => setImmediate(r));
  assert.deepStrictEqual(reactions.map((r) => [r.emoji, r.user.id, r.message.id]), [["👍", "3", "m1"]]);
  assert.deepStrictEqual(custom.map((r) => r.emoji), ["<:party:99>"]);

  const edits = listen("fluxer.on_message_change", { when: "edit" });
  const deletes = listen("fluxer.on_message_change", { when: "delete" });
  client.emit("messageUpdate", message("old"), message("new"));
  client.emit("messageUpdate", message("same"), message("same"));
  client.emit("messageDelete", { id: "m3", partial: true, content: null, author: null, channelId: "c1", guildId: "g1" });
  await new Promise((r) => setImmediate(r));
  assert.deepStrictEqual(edits.map((e) => [e.before, e.after]), [["old", "new"]], "only changes of the text");
  assert.deepStrictEqual(deletes.map((d) => [d.messageId, d.before]), [["m3", ""]]);

  const ready = listen("fluxer.on_ready");
  client.emit("ready");
  assert.deepStrictEqual([ready[0].bot.id, ready[0].servers], ["bot", 1]);

  // ---- Messages ----
  const embed = run("fluxer.build_embed", { embed: { title: "T {x}", description: "" }, x: 1 }).embed;
  assert.strictEqual(embed.title, "T 1");
  assert.throws(() => run("fluxer.build_embed", { embed: { color: "#fff" } }), /empty/);

  const replied = await run("fluxer.send_message", { mode: "reply", text: "Hi", message: message("!hi"), embeds: [embed], pingAuthor: false });
  assert.deepStrictEqual(replied.message, { replied: { content: "Hi", embeds: [embed] }, options: { ping: false } });
  const sent = await run("fluxer.send_message", { mode: "send", text: "Yo", channel: "<#900>" });
  assert.strictEqual(sent.message.channelId, "900", "a channel by its mention");
  const dm = await run("fluxer.send_message", { mode: "dm", text: "psst", user: member("4") });
  assert.strictEqual(dm.message.sentTo, "4", "a member works as the user");
  const edited = await run("fluxer.send_message", { mode: "edit", text: "", message: message("x") });
  assert.deepStrictEqual(edited.message.edited, { content: "", embeds: [] });
  await assert.rejects(run("fluxer.send_message", { mode: "send", text: "", channel: "900" }), /Nothing to send/);

  calls.length = 0;
  await run("fluxer.message_action", { message: message("x"), action: "react", emoji: "🎉" });
  await run("fluxer.message_action", { message: message("x"), action: "pin" });
  await run("fluxer.message_action", { message: message("x"), action: "delete", reason: "spam" });
  assert.deepStrictEqual(calls, [["react", "🎉"], ["pin"], ["delete m1", "spam"]]);

  assert.deepStrictEqual((await run("fluxer.get_messages", { channel: channel("c1"), limit: 2 })).messages.map((m) => m.id), ["m2", "m1"]);
  assert.strictEqual((await run("fluxer.get_messages", { channel: channel("c1"), messageId: "m7" })).message.id, "m7");
  assert.strictEqual((await run("fluxer.bulk_delete", { channel: channel("c1"), count: 500 })).deleted, 100, "at most 100");
  const info = run("fluxer.message_info", { message: message("text") });
  assert.deepStrictEqual([info.content, info.author.id, info.server.id, info.pinned], ["text", "7", "g1", false]);

  // ---- Members and roles ----
  const m = member("2", ["r1"]);
  assert.strictEqual((await run("fluxer.change_role", { member: m, action: "toggle", role: "<@&11>" })).has, true, "switched on");
  assert.strictEqual((await run("fluxer.change_role", { member: m, action: "remove", role: { id: "r1" } })).has, false);
  assert.deepStrictEqual([m.roles.has("11"), m.roles.has("r1")], [true, false]);
  assert.strictEqual(run("fluxer.check_member", { member: m, check: "permission", permission: "BanMembers" }).result, true);
  assert.strictEqual(run("fluxer.check_member", { member: m, check: "permission", permission: "ManageRoles" }).result, false);
  assert.strictEqual(run("fluxer.check_member", { member: m, check: "role", role: "11" }).result, true);

  calls.length = 0;
  await run("fluxer.moderate", { action: "kick", who: member("2"), reason: "rude" });
  await run("fluxer.moderate", { action: "tempban", who: "<@3>", server, minutes: 90, deleteDays: 9 });
  await run("fluxer.moderate", { action: "timeout", who: "4", server, minutes: 5 });
  await run("fluxer.moderate", { action: "untimeout", who: member("4") });
  await run("fluxer.moderate", { action: "unban", who: user("5"), server });
  assert.deepStrictEqual(calls, [
    ["kick 2", "rude"],
    ["ban", "3", { reason: undefined, deleteMessageDays: 7, banDurationSeconds: 5400 }],
    ["timeout 4", 300000, undefined],
    ["timeout 4", null, undefined],
    ["unban", "5", undefined],
  ]);
  await assert.rejects(run("fluxer.moderate", { action: "ban", who: "3" }), /server too/);

  assert.deepStrictEqual(await run("fluxer.find_member", { server, who: "<@!2>" }).then((r) => [r.found, r.member.id]), [true, "2"]);
  assert.strictEqual((await run("fluxer.find_member", { server, who: "404" })).found, false);
  const mi = run("fluxer.member_info", { member: member("2", ["r1"]) });
  assert.deepStrictEqual([mi.name, mi.roles, mi.mention, mi.timedOutUntil], ["Member 2", ["r1"], "<@2>", null]);
  assert.strictEqual(run("fluxer.user_info", { user: member("3") }).username, "user3", "a member turns into its user");
  calls.length = 0;
  await run("fluxer.set_nickname", { member: member("2"), nickname: "  " });
  assert.deepStrictEqual(calls, [["edit 2", { nick: null }]], "empty removes it");

  const role = await run("fluxer.find_role", { server, role: "mod" });
  assert.deepStrictEqual([role.found, role.color, role.mention], [true, "#FF8800", "<@&r1>"]);
  assert.strictEqual((await run("fluxer.find_role", { server, role: "nope" })).found, false);

  // ---- Channels, server, bot ----
  assert.strictEqual((await run("fluxer.find_channel", { channel: "<#900>" })).id, "900");
  assert.strictEqual((await run("fluxer.find_channel", { channel: "#General", server })).id, "c1");
  assert.strictEqual((await run("fluxer.find_channel", { channel: "missing", server })).found, false);
  const created = await run("fluxer.manage_channel", { action: "create", server, name: "logs", kind: "2", parent: "77" });
  assert.deepStrictEqual(created.channel.created, { name: "logs", type: 2, topic: undefined, parentId: "77" });
  const changed = await run("fluxer.manage_channel", { action: "edit", channel: channel("c1"), name: "", topic: "News", slowmode: 5 });
  assert.strictEqual(changed.channel.slowmode, 5);
  const invite = await run("fluxer.create_invite", { channel: channel("c1"), hours: 2, uses: 3 });
  assert.strictEqual(invite.url, "https://fluxer.gg/abc?age=7200&uses=3");
  assert.deepStrictEqual(run("fluxer.server_info", { server }).members, 3);
  assert.deepStrictEqual(run("fluxer.bot_info", {}), { bot: client.user, servers: 1, ping: 42, uptime: 5 });
  calls.length = 0;
  run("fluxer.set_status", { status: "idle", text: "Helping" });
  assert.deepStrictEqual(calls, [["presence", { status: "idle", customStatus: { text: "Helping" } }]]);

  // ---- Formatting ----
  assert.strictEqual(run("fluxer.mention", { target: member("2") }).text, "<@2>");
  assert.strictEqual(run("fluxer.mention", { target: channel("c1") }).text, "<#c1>");
  assert.strictEqual(run("fluxer.mention", { target: { id: "r1", guildId: "g1", position: 1 } }).text, "<@&r1>");
  assert.strictEqual(run("fluxer.mention", { target: "123", kind: "role" }).text, "<@&123>");

  console.log(`${Object.keys(all).length} Fluxer blocks checked`);
  console.log("fluxer blocks test passed");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
