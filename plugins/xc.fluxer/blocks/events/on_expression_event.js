const KINDS = { emojis: "Emojis", stickers: "Stickers" };

module.exports = {
  id: "fluxer.on_expression_event",
  kind: "event",
  info: {
    name: "Emojis or Stickers Changed",
    description: "Runs when a server's emojis or stickers are added, renamed or removed. Gives the whole list as it is now.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "server",
    aliases: ["On emoji", "Emoji added", "Sticker added"],
  },
  look: {
    icon: "star",
  },
  fields: {
    kind: { type: "select", pin: false, default: "emojis", choices: KINDS, label: "Of", description: "Emojis or stickers." },
  },
  outputs: {
    serverId: { type: "text", label: "Server id", description: "The server." },
    list: { type: "list<object>", description: "Every emoji or sticker now: id, name and more." },
    names: { type: "list<text>", description: "Their names." },
    server: { type: "fluxer.server?", description: "The server the emojis or stickers are in." },
  },
  listen(ctx, emit) {
    // The server itself, found in the bot's servers by its id.
    const send = emit;
    emit = (o) => send({ ...o, server: (o.serverId && ctx.fluxer.client.guilds?.get?.(String(o.serverId))) || null });
    const stickers = ctx.fields.kind === "stickers";
    ctx.fluxer.client.on(stickers ? "guildStickersUpdate" : "guildEmojisUpdate", (p) => {
      const list = (stickers ? p?.stickers : p?.emojis) || [];
      emit({ serverId: String(p?.guildId ?? p?.guild_id ?? ""), list, names: list.map((e) => e?.name ?? "") });
    });
  },
};
