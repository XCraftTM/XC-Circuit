const WHEN = { join: "Bot added to a server", leave: "Bot removed from a server" };

module.exports = {
  id: "fluxer.on_server",
  kind: "event",
  info: {
    name: "Server Joined or Left",
    description: "Runs when the bot is added to a server, or removed from one.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Bot added", "Bot removed", "Guild create", "Guild delete"],
  },
  look: {
    icon: "layers",
  },
  fields: {
    when: { type: "select", pin: false, default: "join", choices: WHEN, label: "When", description: "Joining or leaving." },
  },
  outputs: {
    server: { type: "fluxer.server", description: "The server." },
  },
  listen(ctx, emit) {
    // guildCreate fires only for servers joined after the login, not for the ones the bot was in already.
    ctx.fluxer.client.on(ctx.fields.when === "leave" ? "guildDelete" : "guildCreate", (server) => emit({ server }));
  },
};
