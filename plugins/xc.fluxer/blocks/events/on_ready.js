module.exports = {
  id: "fluxer.on_ready",
  kind: "event",
  info: {
    name: "Bot Ready",
    description: "Runs once the bot is logged in to Fluxer and can send messages. (Bot Started runs earlier, before the login.)",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "bot",
    aliases: ["Logged in", "On ready", "Online"],
  },
  look: {
    icon: "bolt",
  },
  outputs: {
    bot: { type: "fluxer.user", description: "The bot's own account." },
    servers: { type: "number", description: "How many servers the bot is in." },
  },
  listen(ctx, emit) {
    const client = ctx.fluxer.client;
    client.once("ready", () => emit({ bot: client.user, servers: client.guilds?.size ?? 0 }));
  },
};
