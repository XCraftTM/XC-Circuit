const KINDS = {
  emojiCreate: "Emoji added",
  emojiUpdate: "Emoji changed",
  emojiDelete: "Emoji deleted",
  stickerCreate: "Sticker added",
  stickerUpdate: "Sticker changed",
  stickerDelete: "Sticker deleted",
};

module.exports = {
  id: "discord.on_expression_event",
  kind: "event",
  info: {
    name: "Emoji / Sticker Changed",
    description: "Runs when a server's custom emoji or sticker is added, changed or deleted.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Emoji added", "Emoji deleted", "Sticker added", "Sticker deleted"],
    tags: ["emoji", "sticker", "expression", "log"],
  },
  look: {
    icon: "message",
  },
  requires: {
    intents: ["Guilds", "GuildExpressions"],
  },
  fields: {
    kind: { type: "select", pin: false, default: "emojiCreate", label: "When", choices: KINDS, description: "Which event runs it." },
  },
  outputs: {
    server: { type: "discord.guild", description: "The server." },
    name: { type: "text", description: "The emoji's or sticker's name (now)." },
    beforeName: { type: "text", label: "Name before", description: "Its name before a change." },
    id: { type: "text", description: "Its id." },
    text: { type: "text", description: "For emojis: the emoji, ready for messages." },
    url: { type: "text", description: "Its picture." },
    item: { type: "object", description: "The emoji or sticker object." },
  },
  listen(ctx, emit) {
    const kind = ctx.fields.kind;
    const out = (item, before = item) =>
      emit({ name: item.name ?? "", beforeName: before.name ?? "", id: item.id, text: kind.startsWith("emoji") ? item.toString() : "", url: item.imageURL?.() ?? item.url ?? "", item, server: item.guild });
    if (kind.endsWith("Update")) ctx.discord.client.on(kind, (old, now) => out(now, old));
    else ctx.discord.client.on(kind, (item) => out(item));
  },
};
