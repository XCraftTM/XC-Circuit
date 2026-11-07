/**
 * Milliseconds a request takes to Fluxer and back. fluxer.js doesn't measure
 * its gateway's ping (as discord.js does), so a request is timed; a ping the
 * library gives is used when there is one.
 */
async function ping(c) {
  const known = c.ws && c.ws.ping;
  if (typeof known === "number" && known >= 0) return Math.round(known);
  if (!c.rest || !c.user) return 0;
  const start = performance.now();
  await c.rest.get("/users/@me");
  return Math.round(performance.now() - start);
}

module.exports = {
  id: "fluxer.bot_info",
  kind: "action",
  info: {
    name: "Bot Info",
    description: "The bot's own account, how many servers it is in, its ping and how long it has been online.",
    author: "XC Team",
    version: "2.0.1",
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
  async run({ ctx }) {
    const c = ctx.fluxer.client;
    return {
      bot: c.user ?? null,
      servers: c.guilds?.size ?? 0,
      ping: await ping(c),
      uptime: Math.round((c.uptime ?? 0) / 1000),
    };
  },
};
