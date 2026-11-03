module.exports = {
  id: "fluxer.bot_info",
  kind: "value",
  info: {
    name: "Bot Info",
    description: "The bot's own account, how many servers it is in, its ping and how long it has been online.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "bot",
    aliases: ["Ping", "Uptime", "Server count", "Latency"],
  },
  look: {
    icon: "info",
  },
  outputs: {
    bot: { type: "fluxer.user?", description: "The bot's account. Empty before it is logged in." },
    servers: { type: "number", description: "How many servers it is in." },
    ping: { type: "number", description: "Milliseconds a message takes to Fluxer and back." },
    uptime: { type: "number", description: "Seconds since it logged in." },
  },
  run({ ctx }) {
    const c = ctx.fluxer.client;
    return {
      bot: c.user ?? null,
      servers: c.guilds?.size ?? 0,
      ping: Math.max(0, Math.round(c.ws?.ping ?? 0)),
      uptime: Math.round((c.uptime ?? 0) / 1000),
    };
  },
};
