// Runtime library for the Fluxer Bot mode: creates the fluxer.js client and
// logs in. Blocks reach it as ctx.fluxer: the client, and helpers they share
// (text commands, ids from mentions, embeds).

module.exports = {
  id: "fluxer",

  setup(rt) {
    const { Client } = rt.require("@fluxerjs/core");
    const client = new Client();
    client.on("error", (e) => rt.log.error(`Fluxer: ${e?.message || e}`));
    return makeApi(client, () => rt.setting("prefix"));
  },

  async start(rt, api) {
    const token = rt.secret("token");
    if (!token) {
      throw new Error("No bot token set. Add it in Bot › Settings (or FLUXER_BOT_TOKEN in .env for exported bots).");
    }
    api.client.once("ready", () => {
      const servers = api.client.guilds?.size ?? 0;
      rt.log.success(`Logged in as ${api.client.user?.username ?? "the bot"} · ${servers} servers`);
    });
    try {
      await api.client.login(token);
    } catch (e) {
      throw new Error(`Login failed: ${e.message}`);
    }
  },
};

/** What blocks get as ctx.fluxer. */
function makeApi(client, prefixSetting) {
  return {
    client,
    /** The text command prefix from Bot › Settings, ! when unset. */
    prefix: () => String(prefixSetting() ?? "").trim() || "!",
    parseCommand,
    id,
    embed,
    /** A channel from a channel or its id (or <#id>). */
    async channel(value) {
      if (value && typeof value === "object" && typeof value.send === "function") return value;
      const channelId = id(value, "channel");
      if (!channelId) throw new Error("Connect a channel or type its id");
      return client.channels.resolve(channelId);
    },
    /** A user from a user, member or id (or <@id>). */
    async user(value) {
      if (value && typeof value === "object" && value.username !== undefined) return value;
      if (value && typeof value === "object" && value.user) return value.user;
      const userId = id(value, "user");
      if (!userId) throw new Error("Connect a user or type their id");
      return client.users.resolve(userId);
    },
    /** A server member from a member, or a user or id together with the server. */
    async member(value, server) {
      if (value && typeof value === "object" && value.roles && value.guild) return value;
      const userId = id(value, "user");
      if (!userId) throw new Error("Connect a member or type their id");
      if (!server) throw new Error("Connect the server too, to find a member by id");
      return server.fetchMember(userId);
    },
  };
}

/**
 * A text command in a message: `!ban @someone spam` with prefix `!` gives
 * { command: "ban", args: ["@someone", "spam"], rest: "@someone spam" }.
 * null when the message doesn't start with the prefix.
 */
function parseCommand(content, prefix) {
  const text = String(content ?? "");
  if (!prefix || !text.startsWith(prefix)) return null;
  const body = text.slice(prefix.length).trim();
  const command = body.split(/\s+/)[0]?.toLowerCase() ?? "";
  if (!command) return null;
  const rest = body.slice(command.length).trim();
  return { command, args: rest ? rest.split(/\s+/) : [], rest };
}

/**
 * The id in a value: an object's id, or a mention or plain id in text
 * (`<@1>`, `<@!1>`, `<@&2>`, `<#3>`, `1`). `kind` picks which mention fits.
 */
function id(value, kind) {
  if (value == null || value === "") return null;
  if (typeof value === "object") return value.id != null ? String(value.id) : null;
  const text = String(value).trim();
  const patterns = { user: /^<@!?(\d+)>$/, role: /^<@&(\d+)>$/, channel: /^<#(\d+)>$/ };
  const m = (patterns[kind] || /^<[@#][!&]?(\d+)>$/).exec(text);
  if (m) return m[1];
  return /^\d+$/.test(text) ? text : null;
}

/**
 * An embed as Fluxer takes it, from the Embed Builder's design. Texts are
 * passed through `fill` (placeholders); the color may be #RRGGBB.
 */
function embed(design, fill = (s) => s) {
  const e = design || {};
  const t = (s) => (s ? String(fill(String(s))) : "");
  const out = { description: t(e.description).slice(0, 4096) || null };
  if (e.title) out.title = t(e.title).slice(0, 256);
  if (e.url) out.url = t(e.url);
  if (e.color) {
    const n = typeof e.color === "number" ? e.color : parseInt(String(e.color).replace(/^#/, ""), 16);
    if (Number.isFinite(n)) out.color = n;
  }
  if (e.author?.name) out.author = { name: t(e.author.name), icon_url: e.author.icon ? t(e.author.icon) : undefined, url: e.author.url ? t(e.author.url) : undefined };
  const fields = (e.fields || []).filter((f) => f.name || f.value).slice(0, 25);
  if (fields.length) out.fields = fields.map((f) => ({ name: t(f.name) || "​", value: t(f.value) || "​", inline: Boolean(f.inline) }));
  if (e.footer?.text) out.footer = { text: t(e.footer.text), icon_url: e.footer.icon ? t(e.footer.icon) : undefined };
  if (e.image) out.image = { url: t(e.image) };
  if (e.thumbnail) out.thumbnail = { url: t(e.thumbnail) };
  if (e.time) {
    const value = t(e.time).trim();
    const date = new Date(/^\d+$/.test(value) ? Number(value) : value);
    if (!Number.isNaN(date.getTime())) out.timestamp = date.toISOString();
  } else if (e.timestamp) out.timestamp = new Date().toISOString();
  return out;
}

// For tests. (Not `module.exports.id`: that is the library's name, "fluxer".)
module.exports.helpers = { makeApi, parseCommand, id, embed };
