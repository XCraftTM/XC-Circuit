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

/** A Slash Command Builder command in the shape Discord's API expects. */
function toDiscordCommand(c) {
  const options = (c.options || [])
    // Discord requires required options before optional ones.
    .slice()
    .sort((a, b) => Number(Boolean(b.required)) - Number(Boolean(a.required)))
    .map((o) => {
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
      return out;
    });
  return { name: c.name, description: String(c.description || "A command made with XC Circuit").slice(0, 100), options };
}

module.exports.toDiscordCommand = toDiscordCommand;

/** Registers every Slash Command block's command, on the dev server or globally. */
async function registerCommands(rt, client) {
  const commands = new Map();
  // Commands from the Slash Command Builder, with their options.
  for (const c of (rt.bundle.project.data && rt.bundle.project.data.slashCommands) || []) {
    if (c.name) commands.set(c.name, toDiscordCommand(c));
  }
  // Commands only named on a block still register, without options.
  for (const node of rt.nodesOf("discord.on_slash_command")) {
    const name = String(node.inline().command || "").trim().toLowerCase();
    if (name && !commands.has(name)) commands.set(name, { name, description: "A command made with XC Circuit" });
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
