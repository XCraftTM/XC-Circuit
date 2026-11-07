module.exports = {
  id: "discord.on_message_edit",
  kind: "event",
  info: {
    name: "Message Edited",
    description: "Runs when someone edits a message the bot can see. For edit logs.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "messages",
    aliases: ["Message updated"],
    tags: ["log", "changed", "update"],
  },
  look: {
    icon: "message",
  },
  requires: {
    intents: ["Guilds", "GuildMessages", "MessageContent"],
  },
  fields: {
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Skip messages written by bots." },
  },
  outputs: {
    message: { type: "discord.message", description: "The message as it is now." },
    before: { type: "text", description: "The text before the edit (empty when the message was sent before the bot started)." },
    after: { type: "text", description: "The text now." },
    author: { type: "discord.user", description: "Who wrote it." },
    channel: { type: "discord.channel", description: "Where it is." },
    server: { type: "discord.guild?", description: "The server it was edited in. Empty in DMs." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("messageUpdate", async (old, now) => {
      if (now.partial) {
        try {
          await now.fetch();
        } catch {
          return;
        }
      }
      if (!now.author || (ctx.fields.ignoreBots && now.author.bot)) return;
      // Embeds loading in also fires this; only text changes count.
      if (!old.partial && old.content === now.content) return;
      emit({ message: now, before: old.partial ? "" : old.content ?? "", after: now.content ?? "", author: now.author, channel: now.channel, server: now.guild ?? null });
    });
  },
};
