// Runtime library for the Discord Bot mode: creates the discord.js client with the
// intents the project's event blocks need, logs in, and registers slash commands.
// Blocks reach it as ctx.discord.

module.exports = {
  id: "discord",

  setup(rt) {
    const { Client, GatewayIntentBits, Partials } = rt.require("discord.js");
    const names = rt.intents();
    const unknown = names.filter((n) => !(n in GatewayIntentBits));
    if (unknown.length) rt.log.warn(`Unknown intents ignored: ${unknown.join(", ")}`);
    const intents = names.filter((n) => n in GatewayIntentBits).map((n) => GatewayIntentBits[n]);
    if (!intents.length) intents.push(GatewayIntentBits.Guilds);
    const client = new Client({ intents, partials: [Partials.Channel, Partials.Message, Partials.Reaction] });
    rt.log.info(`Intents: ${names.length ? names.join(", ") : "Guilds"}`);
    return { client };
  },

  async start(rt, api) {
    const { Events } = rt.require("discord.js");
    const token = rt.secret("token");
    if (!token) {
      throw new Error("No bot token set. Add it in Bot › Settings (or DISCORD_TOKEN in .env for exported bots).");
    }
    api.client.once(Events.ClientReady, async (client) => {
      rt.log.success(`Logged in as ${client.user.tag} · ${client.guilds.cache.size} servers`);
      try {
        await registerCommands(rt, client);
      } catch (e) {
        rt.log.error(`Registering slash commands failed: ${e.message}`);
      }
    });
    try {
      await api.client.login(token);
    } catch (e) {
      const hint = /disallowed intents/i.test(e.message)
        ? " Turn on the privileged intents for your bot in the Discord developer portal (Bot › Privileged Gateway Intents)."
        : "";
      throw new Error(`Login failed: ${e.message}.${hint}`);
    }
  },
};

// Discord's numbers for option types, places, install types and channel kinds.
const OPTION_TYPES = { string: 3, integer: 4, boolean: 5, user: 6, channel: 7, role: 8, mentionable: 9, number: 10, attachment: 11 };
const CONTEXTS = { guild: 0, botDm: 1, privateChannel: 2 };
const INSTALLS = { guild: 0, user: 1 };
const CHANNEL_TYPES = { text: 0, voice: 2, category: 4, announcement: 5, announcementThread: 10, publicThread: 11, privateThread: 12, stage: 13, forum: 15, media: 16 };

const describe = (text) => {
  const t = String(text || "").trim();
  return t ? [...t].slice(0, 100).join("") : "A command made with XC Circuit";
};
const words = (c) => String(c.name || "").trim().split(/\s+/).filter(Boolean);

/** Translations ({ de: { name, description } }) as Discord's name and description localizations. */
function putLocales(out, locales, withDescription = true) {
  const pick = (key) => Object.fromEntries(Object.entries(locales || {}).filter(([, l]) => l && l[key]).map(([code, l]) => [code, l[key]]));
  const names = pick("name");
  if (Object.keys(names).length) out.name_localizations = names;
  if (withDescription) {
    const descriptions = pick("description");
    if (Object.keys(descriptions).length) out.description_localizations = descriptions;
  }
  return out;
}

/** A Slash Command Builder option in the shape Discord's API expects. */
function toDiscordOption(o) {
  const numeric = o.type === "integer" || o.type === "number";
  const text = o.type === "string";
  const out = { type: OPTION_TYPES[o.type] || 3, name: o.name, description: describe(String(o.description || "").trim() ? o.description : o.name) };
  putLocales(out, o.locales);
  out.required = Boolean(o.required);
  if ((o.choices || []).length && (text || numeric)) {
    out.choices = o.choices.slice(0, 25).map((ch) => putLocales({ name: String(ch.name), value: numeric ? Number(ch.value) || 0 : String(ch.value ?? "") }, ch.locales, false));
  }
  if (numeric && o.min != null) out.min_value = Number(o.min);
  if (numeric && o.max != null) out.max_value = Number(o.max);
  // Older projects kept a text option's longest length in max.
  const maxLength = o.maxLength ?? (text ? o.max : undefined);
  if (text && o.minLength != null) out.min_length = Number(o.minLength);
  if (text && maxLength != null) out.max_length = Math.max(1, Number(maxLength));
  if (o.type === "channel" && (o.channelTypes || []).length) out.channel_types = o.channelTypes.map((t) => CHANNEL_TYPES[t]).filter((n) => n !== undefined);
  if (o.autocomplete && !out.choices && (text || numeric)) out.autocomplete = true;
  return out;
}

// Discord requires required options before optional ones.
const optionsOf = (c) => (c.options || []).slice().sort((a, b) => Number(Boolean(b.required)) - Number(Boolean(a.required))).map(toDiscordOption);

const sub = (c, type, name, options) => putLocales({ type, name, description: describe(c.description), options }, c.locales);

/**
 * The Slash Command Builder's commands in the shape Discord's API expects.
 * Names with spaces are subcommands: "ticket open" and "ticket close" become
 * /ticket with two subcommands, and three words make a subcommand group. A
 * name others continue ("ticket", "ticket admin") gives that command or group
 * its description and translations. Who can use a command and where come
 * from the command itself, or its first subcommand.
 * `permissionBits` turns a permission name into its bit string.
 * The editor builds the same JSON (slash.rs); test/fixtures/slash.json checks both.
 */
function toDiscordCommands(list, permissionBits = () => undefined) {
  list = list.filter((c) => words(c).length);
  const roots = [...new Set(list.map((c) => words(c)[0]))];
  const isHeader = (name) => list.some((o) => o.name.startsWith(`${name} `));
  return roots.map((root) => {
    const entries = list.filter((c) => words(c)[0] === root);
    const me = entries.find((c) => words(c).length === 1);
    const nested = entries.filter((c) => words(c).length === 2 && !isHeader(c.name)).map((c) => sub(c, 1, words(c)[1], optionsOf(c)));
    const groups = [...new Set(entries.filter((c) => words(c).length >= 3).map((c) => words(c)[1]))];
    for (const g of groups) {
      const subs = entries.filter((c) => words(c).length >= 3 && words(c)[1] === g).map((c) => sub(c, 1, words(c)[2], optionsOf(c)));
      const header = entries.find((c) => c.name === `${root} ${g}`) || { description: g };
      nested.push(sub(header, 2, g, subs));
    }
    const settings = me || entries[0];
    const description = me && String(me.description || "").trim() ? describe(me.description) : describe(nested[0] && nested[0].description);
    const out = { name: root, description };
    if (me) putLocales(out, me.locales);
    out.options = nested.length ? nested : me ? optionsOf(me) : [];
    const names = (settings.permissions || []).length ? settings.permissions : settings.permission ? [settings.permission] : [];
    const bits = names.map(permissionBits).filter((b) => b !== undefined).reduce((all, b) => all | BigInt(b), 0n);
    if (bits) out.default_member_permissions = bits.toString();
    // Older projects had DMs on or off.
    const contexts = (settings.contexts || []).length ? settings.contexts : settings.dm ? ["guild", "botDm"] : ["guild"];
    out.contexts = contexts.map((c) => CONTEXTS[c]).filter((n) => n !== undefined);
    if (!out.contexts.length) out.contexts = [0];
    if ((settings.installs || []).length) out.integration_types = settings.installs.map((c) => INSTALLS[c]).filter((n) => n !== undefined);
    if (settings.nsfw) out.nsfw = true;
    return out;
  });
}

/** One Builder command (kept for older callers): its root command alone. */
function toDiscordCommand(c) {
  return toDiscordCommands([c])[0];
}

module.exports.toDiscordCommands = toDiscordCommands;
module.exports.toDiscordCommand = toDiscordCommand;

/** Registers every Slash Command block's command, on the dev server or globally. */
async function registerCommands(rt, client) {
  const commands = new Map();
  // Commands from the Slash Command Builder, with their options and subcommands.
  const { PermissionFlagsBits } = rt.require("discord.js");
  const bits = (name) => (name in PermissionFlagsBits ? PermissionFlagsBits[name].toString() : undefined);
  for (const c of toDiscordCommands((rt.bundle.project.data && rt.bundle.project.data.slashCommands) || [], bits)) {
    commands.set(c.name, c);
  }
  // Commands only named on a block still register, without options.
  for (const node of rt.nodesOf("discord.on_slash_command")) {
    const name = String(node.inline().command || "").trim().toLowerCase().split(/\s+/)[0];
    if (name && !commands.has(name)) commands.set(name, { name, description: "A command made with XC Circuit" });
  }
  // Context Menu Used blocks add right-click entries (type 2: users, 3: messages).
  for (const node of rt.nodesOf("discord.on_context_menu")) {
    const v = node.inline();
    const name = String(v.name || "").trim().slice(0, 32);
    const type = v.target === "message" ? 3 : 2;
    if (name) commands.set(`menu:${type}:${name}`, { name, type });
  }
  if (!commands.size) return;
  const list = [...commands.values()];
  const devServer = String(rt.setting("devServer") || "").trim();
  if (devServer) {
    await client.application.commands.set(list, devServer);
    rt.log.info(`Registered ${list.length} slash commands on the dev server`);
  } else {
    await client.application.commands.set(list);
    rt.log.info(`Registered ${list.length} slash commands globally`);
  }
}
