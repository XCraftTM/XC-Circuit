module.exports = {
  id: "fluxer.on_server_update",
  kind: "event",
  info: {
    name: "Server Changed",
    description: "Runs when a server's name, icon or settings change.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["On server update", "Server renamed", "Guild update"],
  },
  look: {
    icon: "shield",
  },
  fields: {},
  outputs: {
    server: { type: "fluxer.server", description: "The server, as it is now." },
    before: { type: "fluxer.server?", description: "The server as it was." },
    nameBefore: { type: "text", label: "Name before", description: "Its name before." },
    name: { type: "text", description: "Its name now." },
  },
  listen(ctx, emit) {
    ctx.fluxer.client.on("guildUpdate", (before, server) => emit({ server, before: before ?? null, nameBefore: before?.name ?? "", name: server?.name ?? "" }));
  },
};
