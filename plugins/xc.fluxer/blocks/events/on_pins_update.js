module.exports = {
  id: "fluxer.on_pins_update",
  kind: "event",
  info: {
    name: "Pins Changed",
    description: "Runs when a message is pinned or unpinned in a channel.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "messages",
    aliases: ["On pin", "Message pinned", "Pins updated"],
  },
  look: {
    icon: "star",
  },
  fields: {},
  outputs: {
    server: { type: "fluxer.server?", description: "The server it happened in. Empty in direct messages." },
    channelId: { type: "text", label: "Channel id", description: "Where the pins changed." },
    serverId: { type: "text", label: "Server id", description: "The server. Empty in direct messages." },
    lastPin: { type: "date?", label: "Last pin", description: "When the newest pin was made, when known." },
  },
  listen(ctx, emit) {
    // The server itself, found in the bot's servers by its id.
    const send = emit;
    emit = (o) => send({ ...o, server: (o.serverId && ctx.fluxer.client.guilds?.get?.(String(o.serverId))) || null });
    ctx.fluxer.client.on("channelPinsUpdate", (p) => {
      const at = p?.lastPinTimestamp ?? p?.last_pin_timestamp;
      emit({ channelId: String(p?.channelId ?? p?.channel_id ?? ""), serverId: String(p?.guildId ?? p?.guild_id ?? ""), lastPin: at ? new Date(at) : null });
    });
  },
};
