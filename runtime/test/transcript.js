// Create Transcript with the real better-discord-transcripts and discord.js
// objects built offline from Discord's own data (no login): every message,
// oldest first; a limit and leaving out bots; no scripts from the web; a
// name with quotes and tags can't put code into the file. Needs both
// packages in XC_TEST_DEPS; without them, this says so and passes.
// Usage: XC_TEST_DEPS=<dir> node runtime/test/transcript.js
"use strict";

const path = require("path");
const Module = require("module");
const assert = require("assert");

let req;
try {
  req = Module.createRequire(path.join(path.resolve(process.env.XC_TEST_DEPS || "."), "package.json"));
  req.resolve("better-discord-transcripts");
  req.resolve("discord.js");
} catch {
  console.log("transcript test skipped: better-discord-transcripts and discord.js aren't in XC_TEST_DEPS");
  process.exit(0);
}
const { Client, GatewayIntentBits, Collection } = req("discord.js");
const block = require(path.join(__dirname, "..", "..", "plugins", "xc.discord", "blocks", "channels", "transcript.js"));
const client = new Client({ intents: [GatewayIntentBits.Guilds] });
const guild = client.guilds._add({ id: "100", name: "XC Lounge", icon: null, roles: [], channels: [], members: [], emojis: [], stickers: [] });
const channel = client.channels._add({ id: "200", type: 0, name: "ticket-0001", guild_id: "100" }, guild);
const author = (id, name, bot = false, global = name) => ({ id, username: name, global_name: global, discriminator: "0", avatar: null, bot });
const raw = (id, a, content) => ({ id, channel_id: "200", author: a, content, timestamp: `2026-11-06T10:0${id % 10}:00.000Z`, type: 0, attachments: [], embeds: [], mentions: [], mention_roles: [], pinned: false, tts: false, mention_everyone: false, edited_timestamp: null });
const all = [raw(305, author("9", "TicketBot", true), "Ticket closed"), raw(304, author("2", "helper"), "Fixed!"), raw(303, author("1", "alex"), "Still waiting"), raw(302, author("2", "helper"), "Checking"), raw(301, author("1", "alex", false, 'Alex" onerror="alert(1)</script><script>alert(2)</script>'), "Help please <script>alert(3)</script>")];
// Discord gives newest first, 100 at a time.
channel.messages.fetch = async ({ limit, before }) => {
  const list = all.filter((m) => !before || BigInt(m.id) < BigInt(before)).slice(0, limit).map((r) => channel.messages._add(r));
  return new Collection(list.map((m) => [m.id, m]));
};
const ctx = { require: (n) => req(n) };
(async () => {
  const a = await block.run({ fields: { channel, limit: 0, name: "transcript-${channel.name}", images: false, bots: true }, ctx });
  const b = await block.run({ fields: { channel, limit: 2, name: "t.html", bots: false }, ctx });
  // The renderer's <!-- --> markers split words; removed until none is left.
  const strip = (b) => {
    let s = b.toString();
    for (let before = null; before !== s; ) {
      before = s;
      s = s.replace(/<!--[\s\S]*?-->/g, "");
    }
    return s;
  };
  const raw1 = a.file.attachment.toString();
  const html = strip(a.file.attachment);
  assert.deepStrictEqual([a.file.name, a.count], ["transcript-${channel.name}.html", 5]);
  for (const s of ["Help please", "Checking", "Still waiting", "Fixed!", "Ticket closed"]) assert.ok(html.includes(s), s);
  assert.ok(html.indexOf("Help please") < html.indexOf("Fixed!"), "oldest first");
  const h2 = strip(b.file.attachment);
  assert.deepStrictEqual([b.file.name, b.count], ["t.html", 2]);
  assert.ok(h2.includes("Fixed!") && h2.includes("Still waiting") && !h2.includes("Ticket closed") && !h2.includes("Help please"), "the 2 latest, without the bot's");
  assert.ok(!/<script[^>]+src=/i.test(raw1), "no scripts from the web");
  assert.ok(!raw1.includes("window.$discordMessage={"), "no profiles in a script");
  // Names and messages stay text: no tag gets an onerror, no <script> outside attribute values.
  const outsideValues = raw1.replace(/="[^"]*"/g, '=""');
  assert.ok(!/<[a-z][^>]*\sonerror=/i.test(outsideValues), "no tag gets an onerror from a name");
  assert.ok(raw1.includes('alt="Alex&quot; onerror=&quot;'), "a quote in a name stays inside its attribute");
  assert.ok(!/<script>alert/.test(outsideValues), "no script from a name or a message");
  console.log("transcript test passed");
  client.destroy();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
