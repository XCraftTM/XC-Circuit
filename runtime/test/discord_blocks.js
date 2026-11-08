// Checks the Discord blocks without Discord: every block file is well formed,
// and the blocks that only work with text and dates give the right results.
// Usage: node runtime/test/discord_blocks.js
"use strict";

const fs = require("fs");
const path = require("path");
const assert = require("assert");
const { ungroupBlock } = require("../circuit-runtime.js");

const dir = path.join(__dirname, "..", "..", "plugins", "xc.discord", "blocks");
const all = {};
for (const group of fs.readdirSync(dir)) {
  for (const f of fs.readdirSync(path.join(dir, group))) {
    const block = ungroupBlock(require(path.join(dir, group, f)));
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

// Build Layout: placeholders are filled in every text, and pictures and files
// that are not on the web are attached.
{
  const layout = {
    components: [
      { type: 17, accent_color: 1, components: [{ type: 10, content: "Hi {who}" }, { type: 9, components: [{ type: 10, content: "x" }], accessory: { type: 11, media: { url: "C:/pics/cat.png" } } }] },
      { type: 12, items: [{ media: { url: "https://x.example/a.png" } }, { media: { url: "/tmp/cat.png" } }] },
      { type: 13, file: { url: "attachment://log.txt" } },
    ],
  };
  const fill = (s, f) => s.replace(/\{(\w+)\}/g, (w, k) => (k in f ? String(f[k]) : w));
  const out = all["discord.build_layout"].run({ fields: { layout, who: "Ada" }, ctx: { fill } }).layout;
  assert.strictEqual(out.components[0].components[0].content, "Hi Ada");
  assert.strictEqual(out.components[0].components[1].accessory.media.url, "attachment://cat.png");
  assert.strictEqual(out.components[1].items[0].media.url, "https://x.example/a.png");
  assert.strictEqual(out.components[1].items[1].media.url, "attachment://1-cat.png", "two files with one name");
  assert.deepStrictEqual(out.files.map((f) => f.name), ["cat.png", "1-cat.png"]);
  assert.strictEqual(layout.components[0].components[0].content, "Hi {who}", "the design itself is unchanged");
}

// Slash commands: names with spaces are subcommands and groups.
const { toDiscordCommands, registrationPlan, sameCommands, shardOptions } = require("../../plugins/xc.discord/runtime/discord.js");

// The Shards setting.
assert.deepStrictEqual(shardOptions(""), {});
assert.deepStrictEqual(shardOptions("1"), {}, "one shard is no sharding");
assert.deepStrictEqual(shardOptions(" Auto "), { shards: "auto" });
assert.deepStrictEqual(shardOptions("3"), { shards: [0, 1, 2], shardCount: 3 });

// Registering slash commands: only when something changed, and old ones are cleared.
{
  const ping = [{ name: "ping", description: "Pong" }];
  const first = registrationPlan(ping, "global", null);
  assert.deepStrictEqual(first.steps, [{ scope: "global", list: ping }]);
  assert.deepStrictEqual(registrationPlan(ping, "global", first.record).steps, [], "unchanged commands aren't sent again");
  assert.deepStrictEqual(registrationPlan([], "global", first.record).steps, [{ scope: "global", list: [] }], "deleting the last command clears it");
  assert.deepStrictEqual(registrationPlan([], "global", null).steps, [], "nothing registered, nothing to clear");
  assert.deepStrictEqual(
    registrationPlan(ping, "123", first.record).steps,
    [{ scope: "global", list: [] }, { scope: "123", list: ping }],
    "moving to the dev server clears the global ones",
  );
  // With what Discord has: sent again when it differs, whatever was sent last time.
  const ours = [{ name: "ping", description: "Pong", options: [{ type: 3, name: "who", description: "Who", required: false }], contexts: [0, 1] }];
  // Discord's answer: defaults left out, ids and versions added, contexts in its order.
  const discord = [{ id: "1", application_id: "2", version: "3", type: 1, name: "ping", description: "Pong", options: [{ type: 3, name: "who", description: "Who" }], contexts: [1, 0], integration_types: [0], default_member_permissions: null, nsfw: false, name_localizations: null }];
  assert.ok(sameCommands(discord, ours), "the same commands, as Discord returns them");
  assert.deepStrictEqual(registrationPlan(ours, "global", first.record, discord).steps, [], "in sync: nothing sent");
  const edited = [{ ...discord[0], description: "Changed in the portal" }];
  assert.deepStrictEqual(registrationPlan(ours, "global", registrationPlan(ours, "global", null).record, edited).steps, [{ scope: "global", list: ours }], "changed on Discord: sent again");
  assert.deepStrictEqual(registrationPlan(ours, "global", registrationPlan(ours, "global", null).record, []).steps, [{ scope: "global", list: ours }], "removed on Discord: sent again");
  assert.deepStrictEqual(registrationPlan([], "global", null, discord).steps, [{ scope: "global", list: [] }], "left on Discord but deleted here: cleared");
  assert.deepStrictEqual(registrationPlan([], "global", null, []).steps, [], "nothing on either side");
  const menu = [{ name: "Report", type: 3 }];
  assert.ok(sameCommands([{ id: "4", type: 3, name: "Report", description: "", contexts: [0, 1, 2], integration_types: [0] }], menu), "a menu entry with Discord's own contexts");
}
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

// Every setting (translations, choices, lengths, places, installs) gives the same
// JSON as the editor's JSON view (crates/circuit-editor/src/slash.rs checks this file too).
const permissionBits = { ManageMessages: "8192", KickMembers: "2", BanMembers: "4" };
assert.deepStrictEqual(
  toDiscordCommands(require("./fixtures/slash-input.json"), (n) => permissionBits[n]),
  require("./fixtures/slash-discord.json"),
);
// Turned-off commands aren't registered.
assert.deepStrictEqual(
  toDiscordCommands([{ name: "ping", description: "P" }, { name: "admin ban", description: "B", disabled: true }]).map((c) => c.name),
  ["ping"],
);

// Show Modal: a box's filled-in text that is empty or shorter than the
// box's least characters is left out (Discord would refuse the whole form).
{
  const { fitTexts } = require(path.join(dir, "interactions", "show_modal.js"));
  const said = [];
  const form = fitTexts(
    [
      { type: 18, label: "Colour", component: { type: 4, custom_id: "c", min_length: 7, value: "#FFF" } },
      { type: 18, label: "Name", component: { type: 4, custom_id: "n", value: "", placeholder: "" } },
      { type: 18, label: "Short", component: { type: 4, custom_id: "s", max_length: 3, value: "abcdef" } },
      { type: 18, label: "Fine", component: { type: 4, custom_id: "f", min_length: 2, value: "ok" } },
    ],
    (t) => said.push(t),
  );
  assert.ok(!("value" in form[0].component) && said[0].includes('"Colour"') && said[0].includes("7"), said.join(" | "));
  assert.ok(!("value" in form[1].component) && !("placeholder" in form[1].component));
  assert.strictEqual(form[2].component.value, "abc");
  assert.strictEqual(form[3].component.value, "ok");
}

// Colours: hex with or without its #, picked or from an input or ${…}.
{
  const fill = (s, f) => s.replace(/\$\{(\w+)\}/g, (w, k) => (k in f ? String(f[k]) : w));
  // Build Embed needs discord.js (there when it's installed next to the tests).
  let discordJs = true;
  try {
    require.resolve("discord.js");
  } catch {
    discordJs = false;
  }
  if (discordJs) {
    const embedColor = (color, extra = {}) => all["discord.build_embed"].run({ fields: { embed: { color, title: "x" }, ...extra }, ctx: { fill } }).embed.data.color;
    assert.strictEqual(embedColor("#FF5CA8"), 0xff5ca8);
    assert.strictEqual(embedColor("ff5ca8"), 0xff5ca8);
    assert.strictEqual(embedColor("#F5A"), 0xff55aa);
    assert.strictEqual(embedColor("${c}", { c: "00D8F0" }), 0x00d8f0);
    assert.strictEqual(embedColor("${c}", { c: "#00D8F0" }), 0x00d8f0);
    assert.strictEqual(embedColor(""), undefined, "no colour");
    assert.throws(() => embedColor("${c}", { c: "red" }), /"red" isn't a hex color/);
  }

  const layoutAccent = (accent, extra = {}) => all["discord.build_layout"].run({ fields: { layout: { components: [{ type: 17, accent_color: accent, components: [] }] }, ...extra }, ctx: { fill } }).layout.components[0].accent_color;
  assert.strictEqual(layoutAccent(0x123456), 0x123456, "a picked one stays a number");
  assert.strictEqual(layoutAccent("${c}", { c: "FF5CA8" }), 0xff5ca8);
  assert.strictEqual(layoutAccent("${c}", { c: "" }), undefined, "empty: none");

  (async () => {
    let made, edited;
    const server = { roles: { create: async (o) => (made = o) } };
    for (const [given, want] of [["#99AAB5", 0x99aab5], ["99AAB5", 0x99aab5], [0x112233, 0x112233], ["", undefined]]) {
      await all["discord.create_role"].run({ fields: { ...defaults("discord.create_role"), server, name: "r", color: given } });
      assert.strictEqual(made.color, want, `Create Role with ${given}`);
    }
    await assert.rejects(all["discord.create_role"].run({ fields: { ...defaults("discord.create_role"), server, name: "r", color: "#GG0000" } }), /isn't a hex color/);
    const role = { edit: async (o) => (edited = o) };
    await all["discord.edit_role"].run({ fields: { ...defaults("discord.edit_role"), role, color: "5865f2" } });
    assert.strictEqual(edited.color, 0x5865f2);
    await all["discord.edit_role"].run({ fields: { ...defaults("discord.edit_role"), role, color: "" } });
    assert.ok(!("color" in edited), "empty keeps it");
  })().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}

// Send Message to a channel given by its id (typed in, or <#id>), also where
// the app doesn't look ids up before the block runs.
(async () => {
  const sent = [];
  const channels = { 42: { id: "42", send: async (p) => (sent.push(p.content), { id: "m1" }) } };
  const ctx = { discord: { client: { channels: { fetch: async (id) => channels[id] ?? Promise.reject(new Error("Unknown Channel")) } } } };
  const send = (channel) => all["discord.send_message"].run({ fields: { ...defaults("discord.send_message"), mode: "send", text: "hi", channel }, ctx });
  await send(" 42 ");
  await send("<#42>");
  await send(channels[42]);
  assert.deepStrictEqual(sent, ["hi", "hi", "hi"]);
  await assert.rejects(send("999"), /no channel with the id "999"/);
  await assert.rejects(send("general"), /no channel with the id "general"/);
  // Send to channel uses the channel even with an interaction still wired
  // from when it replied (that interaction has no send).
  const stale = { reply: async () => assert.fail("replied instead of sending"), isRepliable: () => true };
  await all["discord.send_message"].run({ fields: { ...defaults("discord.send_message"), mode: "send", text: "hi", channel: channels[42], interaction: stale }, ctx });
  assert.ok(!stale.xcAnswering, "the interaction is left to be answered");
  // A channel kept as data (a variable, Stored Data): found again by its id.
  await send({ id: "42", name: "general" });
  assert.deepStrictEqual(sent, ["hi", "hi", "hi", "hi", "hi"]);
  // One that takes no messages says so.
  channels[50] = { id: "50", name: "Info" };
  await assert.rejects(send("50"), /Channel "Info": messages can't be sent to it/);
  await assert.rejects(send(null), /Connect the channel/);
  // Delete after: answers to an interaction are deleted through it (that
  // works for private replies too), follow-ups by their id; channel messages
  // as messages. Defer, then delete thinks and takes it away, and a later
  // reply comes as a new message.
  {
    const wait = (ms) => new Promise((r) => setTimeout(r, ms));
    const calls = [];
    const fake = () => ({
      deferred: false,
      replied: false,
      async reply(p) {
        this.replied = true;
        calls.push(`reply${p.flags ? " private" : ""}`);
        return { resource: { message: { id: "r", delete: async () => calls.push("message.delete r") } } };
      },
      async followUp() {
        calls.push("followUp");
        return { id: "f", delete: async () => calls.push("message.delete f") };
      },
      async deferReply() {
        this.deferred = true;
        calls.push("deferReply");
      },
      async editReply() {
        calls.push("editReply");
        return { id: "e", delete: async () => {} };
      },
      async deleteReply(m) {
        calls.push(`deleteReply ${m ? m.id : "@original"}`);
      },
    });
    const sendWith = (fields) => all["discord.send_message"].run({ fields: { ...defaults("discord.send_message"), text: "hi", ...fields }, ctx: { log: { warn: (t) => calls.push(`warn ${t}`) } } });
    await sendWith({ mode: "reply", private: true, deleteAfter: 0.01, interaction: fake() });
    await wait(60);
    assert.deepStrictEqual(calls.splice(0), ["reply private", "deleteReply @original"], "a private reply goes through the interaction");
    await sendWith({ mode: "followup", deleteAfter: 0.01, interaction: fake() });
    await wait(60);
    assert.deepStrictEqual(calls.splice(0), ["followUp", "deleteReply f"]);
    const thinking = fake();
    await sendWith({ mode: "deferDelete", deleteAfter: 0, interaction: thinking });
    assert.deepStrictEqual(calls.splice(0), ["deferReply", "deleteReply @original"], "thinks, then it's gone");
    await sendWith({ mode: "reply", interaction: thinking });
    assert.deepStrictEqual(calls.splice(0), ["followUp"], "after it, a reply is a new message");
    const sentHere = { id: "c", delete: async () => calls.push("message.delete c") };
    await sendWith({ mode: "send", deleteAfter: 0.01, channel: { id: "9", send: async () => sentHere } });
    await wait(60);
    assert.deepStrictEqual(calls.splice(0), ["message.delete c"]);
  }
  console.log(`${Object.keys(all).length} Discord blocks checked\ndiscord blocks test passed`);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
