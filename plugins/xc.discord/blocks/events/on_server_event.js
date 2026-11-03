const KINDS = { joined: "The bot is added to a server", left: "The bot is removed from a server" };

module.exports = {
  id: "discord.on_server_event",
  kind: "event",
  info: {
    name: "Bot Added to Server",
    description: "Runs when the bot joins a server, or is removed from one.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_server",
    aliases: ["Guild join", "Bot invited"],
    tags: ["guild create", "invite", "kicked", "joined"],
  },
  look: {
    icon: "layers",
  },
  requires: {
    intents: ["Guilds"],
  },
  fields: {
    kind: { type: "select", pin: false, default: "joined", label: "When", choices: KINDS, description: "Added or removed." },
  },
  outputs: {
    server: { type: "discord.guild", description: "The server." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on(ctx.fields.kind === "left" ? "guildDelete" : "guildCreate", (server) => emit({ server }));
  },
};
