const INFO = {
  name: ["Name", "text", (s) => s.name],
  id: ["Id", "text", (s) => s.id],
  memberCount: ["Member count", "number", (s) => s.memberCount],
  owner: ["Owner", "discord.member", (s) => s.fetchOwner()],
  createdAt: ["Created at", "date", (s) => s.createdAt],
  icon: ["Icon URL", "text", (s) => s.iconURL({ size: 512 }) ?? ""],
  banner: ["Banner URL", "text", (s) => s.bannerURL({ size: 1024 }) ?? ""],
  description: ["Description", "text", (s) => s.description ?? ""],
  boostLevel: ["Boost level", "number", (s) => s.premiumTier],
  boosts: ["Boost count", "number", (s) => s.premiumSubscriptionCount ?? 0],
  channels: ["Channels", "list<discord.channel>", (s) => [...s.channels.cache.values()]],
  channelCount: ["Channel count", "number", (s) => s.channels.cache.size],
  roleCount: ["Role count", "number", (s) => s.roles.cache.size],
  systemChannel: ["System channel", "discord.channel?", (s) => s.systemChannel],
  rulesChannel: ["Rules channel", "discord.channel?", (s) => s.rulesChannel],
  everyone: ["@everyone role", "discord.role", (s) => s.roles.everyone],
  bot: ["The bot as a member", "discord.member", (s) => s.members.me],
};
const CHOICES = Object.fromEntries(Object.entries(INFO).map(([k, [label]]) => [k, label]));

module.exports = {
  id: "discord.server_info",
  kind: "value",
  info: {
    name: "Server Info",
    description: "Reads one piece of information about a server.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    tags: ["guild", "serverinfo", "members count"],
  },
  look: {
    icon: "layers",
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    info: { type: "select", pin: false, default: "name", choices: CHOICES, description: "What to read." },
  },
  outputs: {
    result: { type: "text", description: "The value you picked." },
  },
  define({ values }) {
    const [label, type] = INFO[values.info] || INFO.name;
    return { outputs: { result: { type, label, description: `The server's ${label.toLowerCase()}.` } } };
  },
  async run({ fields }) {
    const [, , read] = INFO[fields.info] || INFO.name;
    return { result: await read(fields.server) };
  },
};
