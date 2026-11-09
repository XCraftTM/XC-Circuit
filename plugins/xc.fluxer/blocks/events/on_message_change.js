const WHEN = { edit: "Edited", delete: "Deleted" };

module.exports = {
  id: "fluxer.on_message_change",
  kind: "event",
  info: {
    name: "Message Edited or Deleted",
    description: "Runs when a message is edited or deleted. Messages from before the bot started may only give their id.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "messages",
    aliases: ["Message edited", "Message deleted", "On edit", "On delete"],
  },
  look: {
    icon: "message",
  },
  fields: {
    when: { type: "select", pin: false, default: "edit", choices: WHEN, label: "When", description: "Edited or deleted messages." },
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Skip messages sent by bots." },
  },
  outputs: {
    message: { type: "fluxer.message?", description: "The message as it is now (edits). Empty for deleted messages." },
    author: { type: "fluxer.user?", description: "Who wrote it, when known." },
    server: { type: "fluxer.server?", description: "The server it happened in. Empty in direct messages." },
    before: { type: "text", description: "Its text before the edit, or the deleted text, when the bot had seen it. Otherwise empty." },
    after: { type: "text", description: "Its text after the edit. Empty for deleted messages." },
    messageId: { type: "text", label: "Message id", description: "The message's id." },
    channelId: { type: "text", label: "Channel id", description: "Where it was." },
    serverId: { type: "text", label: "Server id", description: "The server it was in. Empty in direct messages." },
  },
  listen(ctx, emit) {
    // The server itself, found in the bot's servers by its id.
    const send = emit;
    emit = (o) => send({ ...o, server: (o.serverId && ctx.fluxer.client.guilds?.get?.(String(o.serverId))) || null });
    const bot = (author) => ctx.fields.ignoreBots && author?.bot;
    if (ctx.fields.when === "delete") {
      ctx.fluxer.client.on("messageDelete", (message) => {
        if (bot(message.author)) return;
        emit({ message: null, before: message.content ?? "", after: "", messageId: message.id, author: message.author ?? null, channelId: message.channelId ?? "", serverId: message.guildId ?? "" });
      });
      return;
    }
    ctx.fluxer.client.on("messageUpdate", (old, now) => {
      if (bot(now.author ?? old?.author)) return;
      // A partial message has to be fetched before its text is known.
      Promise.resolve(now.partial ? now.fetch().catch(() => now) : now).then((message) => {
        if (old?.content === message.content) return; // an embed or pin changed, not the text
        emit({
          message: message.partial ? null : message,
          before: old?.content ?? "",
          after: message.content ?? "",
          messageId: message.id,
          author: message.author ?? null,
          channelId: message.channelId ?? "",
          serverId: message.guildId ?? "",
        });
      });
    });
  },
};
