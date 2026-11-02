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

// Discord's numbers for option types.
const OPTION_TYPES = { string: 3, integer: 4, boolean: 5, user: 6, channel: 7, role: 8, mentionable: 9, number: 10, attachment: 11 };

/** A Slash Command Builder option in the shape Discord's API expects. */
function toDiscordOption(o) {
  const out = {
    type: OPTION_TYPES[o.type] || 3,
    name: o.name,
    description: String(o.description || o.name).slice(0, 100),
    required: Boolean(o.required),
  };
  const numeric = o.type === "integer" || o.type === "number";
  if ((o.choices || []).length && (o.type === "string" || numeric)) {
    out.choices = o.choices.slice(0, 25).map((ch) => ({ name: String(ch.name), value: numeric ? Number(ch.value) : String(ch.value) }));
  }
  if (numeric && o.min != null) out.min_value = Number(o.min);
  if (numeric && o.max != null) out.max_value = Number(o.max);
  if (o.type === "string" && o.max != null) out.max_length = Number(o.max);
  if (o.autocomplete && !out.choices && (o.type === "string" || numeric)) out.autocomplete = true;
  return out;
}

// Discord requires required options before optional ones.
const optionsOf = (c) => (c.options || []).slice().sort((a, b) => Number(Boolean(b.required)) - Number(Boolean(a.required))).map(toDiscordOption);
const describe = (c) => String((c && c.description) || "A command made with XC Circuit").slice(0, 100);

/**
 * The Slash Command Builder's commands in the shape Discord's API expects.
 * Names with spaces are subcommands: "ticket open" and "ticket close" become
 * /ticket with two subcommands, and three words make a subcommand group.
 * `permissionBits` turns a permission name into its bit string.
 */
function toDiscordCommands(list, permissionBits = () => undefined) {
  const roots = new Map();
  for (const c of list) {
    const words = String(c.name || "").trim().split(/\s+/).filter(Boolean);
    if (!words.length) continue;
    const root = getRoot(roots, words[0]);
    if (words.length === 1) {
      root.self = c;
    } else if (words.length === 2) {
      root.subs.push({ type: 1, name: words[1], description: describe(c), options: optionsOf(c) });
    } else {
      let group = root.groups.get(words[1]);
      if (!group) root.groups.set(words[1], (group = { type: 2, name: words[1], description: words[1], options: [] }));
      group.options.push({ type: 1, name: words[2], description: describe(c), options: optionsOf(c) });
    }
    root.settings = root.settings || (c.permission || c.dm !== undefined ? c : null);
  }
  return [...roots.values()].map((r) => {
    const nested = [...r.subs, ...r.groups.values()];
    const out = { name: r.name, description: describe(r.self || { description: nested[0]?.description }), options: nested.length ? nested : r.self ? optionsOf(r.self) : [] };
    const settings = r.self || r.settings || {};
    const bits = settings.permission ? permissionBits(settings.permission) : undefined;
    if (bits !== undefined) out.default_member_permissions = bits;
    // Contexts: 0 servers, 1 the bot's DMs.
    out.contexts = settings.dm ? [0, 1] : [0];
    return out;
  });
}

function getRoot(roots, name) {
  if (!roots.has(name)) roots.set(name, { name, self: null, subs: [], groups: new Map(), settings: null });
  return roots.get(name);
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
