module.exports = {
  id: "discord.on_message_delete",
  kind: "event",
  info: {
    name: "Message Deleted",
    description: "Runs when a message is deleted, one run per message (also for bulk deletes). For delete logs.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_messages",
    aliases: ["Message removed"],
    tags: ["log", "removed"],
  },
  look: {
    icon: "trash",
  },
  requires: {
    intents: ["Guilds", "GuildMessages", "MessageContent"],
  },
  fields: {
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Skip messages written by bots (when known)." },
  },
  outputs: {
    content: { type: "text", description: "Its text, when it was sent while the bot was running." },
    author: { type: "discord.user?", description: "Who wrote it, when known." },
    channel: { type: "discord.channel", description: "Where it was." },
    id: { type: "text", label: "Message id", description: "The deleted message's id." },
    bulk: { type: "boolean", description: "True when it went in a bulk delete." },
  },
  listen(ctx, emit) {
    const send = (m, bulk) => {
      if (ctx.fields.ignoreBots && m.author?.bot) return;
      emit({ content: m.partial ? "" : m.content ?? "", author: m.author ?? null, channel: m.channel, id: m.id, bulk });
    };
    ctx.discord.client.on("messageDelete", (m) => send(m, false));
    ctx.discord.client.on("messageDeleteBulk", (list) => list.forEach((m) => send(m, true)));
  },
};
