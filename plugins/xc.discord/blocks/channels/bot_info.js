const INFO = {
  ping: ["Ping (ms)", "number", (c) => Math.round(c.ws.ping)],
  uptime: ["Uptime (ms)", "number", (c) => c.uptime ?? 0],
  uptimeText: ["Uptime as text", "text", (c) => {
    let s = Math.floor((c.uptime ?? 0) / 1000);
    const parts = [[86400, "d"], [3600, "h"], [60, "min"]].map(([n, u]) => { const v = Math.floor(s / n); s %= n; return v ? `${v} ${u}` : ""; }).filter(Boolean);
    return parts.length ? parts.join(" ") : `${s} s`;
  }],
  servers: ["Server count", "number", (c) => c.guilds.cache.size],
  serverList: ["Servers", "list<discord.guild>", (c) => [...c.guilds.cache.values()]],
  users: ["Member count (all servers)", "number", (c) => c.guilds.cache.reduce((n, g) => n + (g.memberCount || 0), 0)],
  user: ["The bot's user", "discord.user", (c) => c.user],
  tag: ["Name", "text", (c) => c.user.tag],
  id: ["Id", "text", (c) => c.user.id],
  avatar: ["Avatar URL", "text", (c) => c.user.displayAvatarURL({ size: 512 })],
  invite: ["Invite link", "text", (c) => c.generateInvite({ scopes: ["bot", "applications.commands"], permissions: ["Administrator"] })],
};
const CHOICES = Object.fromEntries(Object.entries(INFO).map(([k, [label]]) => [k, label]));

module.exports = {
  id: "discord.bot_info",
  kind: "action",
  info: {
    name: "Bot Info",
    description: "Reads something about the bot itself: ping, uptime, servers, its user, an invite link.",
    author: "XC Team",
    version: "2.0.0",
  },
  search: {
    category: "bot",
    aliases: ["Ping", "Uptime", "Latency"],
    tags: ["ping", "latency", "uptime", "stats", "invite"],
  },
  look: {
    icon: "bolt",
  },
  fields: {
    info: { type: "select", pin: false, default: "ping", choices: CHOICES, description: "What to read." },
  },
  outputs: {
    result: { type: "number", description: "The value you picked." },
  },
  define({ values }) {
    const [label, type] = INFO[values.info] || INFO.ping;
    return { outputs: { result: { type, label, description: `The bot's ${label.toLowerCase()}.` } } };
  },
  run({ fields, ctx }) {
    const [, , read] = INFO[fields.info] || INFO.ping;
    return { result: read(ctx.discord.client) };
  },
};
