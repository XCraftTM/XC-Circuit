module.exports = {
  id: "fluxer.on_typing",
  kind: "event",
  info: {
    name: "Someone Is Typing",
    description: "Runs when someone starts typing in a channel the bot can see.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "messages",
    aliases: ["On typing", "Typing started"],
  },
  look: {
    icon: "keyboard",
  },
  fields: {
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Skip bots, including this one." },
  },
  outputs: {
    userId: { type: "text", label: "User id", description: "Who is typing." },
    member: { type: "fluxer.member?", description: "Who is typing, as a server member. Empty in direct messages." },
    channelId: { type: "text", label: "Channel id", description: "Where." },
    serverId: { type: "text", label: "Server id", description: "The server. Empty in direct messages." },
    server: { type: "fluxer.server?", description: "The server it happened in. Empty in direct messages." },
  },
  listen(ctx, emit) {
    // The server itself, found in the bot's servers by its id.
    const send = emit;
    emit = (o) => send({ ...o, server: (o.serverId && ctx.fluxer.client.guilds?.get?.(String(o.serverId))) || null });
    ctx.fluxer.client.on("typingStart", (p) => {
      if (ctx.fields.ignoreBots && (p?.member?.user?.bot || String(p?.userId ?? p?.user_id) === String(ctx.fluxer.client.user?.id))) return;
      emit({ userId: String(p?.userId ?? p?.user_id ?? ""), member: p?.member ?? null, channelId: String(p?.channelId ?? p?.channel_id ?? ""), serverId: String(p?.guildId ?? p?.guild_id ?? "") });
    });
  },
};
