const ACTIONS = { list: "List integrations", remove: "Remove an integration" };

const info = (i) => ({ id: String(i.id), name: i.name, type: i.type, enabled: i.enabled !== false, account: i.account?.name ?? "", bot: i.application?.bot?.username ?? "", role: i.role?.id ?? "" });

module.exports = {
  id: "discord.integrations",
  kind: "action",
  info: {
    name: "Integrations",
    description: "Lists a server's integrations (other bots and apps, Twitch, YouTube …) or removes one, which kicks a bot along with its role. The bot needs Manage Server.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["List bots", "Remove integration", "Connected apps"],
    tags: ["integration", "bot", "app", "twitch", "youtube"],
  },
  look: {
    icon: "plug",
  },
  fields: {
    action: { type: "select", pin: false, default: "list", choices: ACTIONS, description: "What to do." },
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    which: { type: "text", default: "", show: { action: ["remove"] }, label: "Integration", description: "Its id or name." },
  },
  outputs: {
    list: { type: "list<object>", description: "Each integration: id, name, type (discord, twitch, youtube …), enabled, account, bot and role." },
    count: { type: "number", description: "How many there are." },
  },
  async run({ fields: f }) {
    const s = f.server;
    if (!s?.fetchIntegrations) throw new Error("Connect a server");
    const all = [...(await s.fetchIntegrations()).values()];
    if (f.action === "remove") {
      const key = String(f.which || "").trim();
      const i = all.find((x) => String(x.id) === key || x.name?.toLowerCase() === key.toLowerCase());
      if (!i) throw new Error(`No integration ${key}`);
      await i.delete();
      const rest = all.filter((x) => x !== i);
      return { list: rest.map(info), count: rest.length };
    }
    return { list: all.map(info), count: all.length };
  },
};
