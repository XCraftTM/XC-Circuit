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

/** Registers every Slash Command block's command, on the dev server or globally. */
async function registerCommands(rt, client) {
  const commands = new Map();
  for (const node of rt.nodesOf("discord.on_slash_command")) {
    const v = node.inline();
    const name = String(v.command || "").trim().toLowerCase();
    if (!name) continue;
    commands.set(name, { name, description: String(v.description || "A command made with XC Circuit").slice(0, 100) });
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
