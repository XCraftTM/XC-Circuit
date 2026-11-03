const emojiText = (e) => (e.id ? `<${e.animated ? "a" : ""}:${e.name}:${e.id}>` : e.name);

module.exports = {
  id: "discord.on_reaction_remove",
  kind: "event",
  info: {
    name: "Reaction Removed",
    description: "Runs when someone takes their reaction back. For reaction roles.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_messages",
    aliases: ["Emoji removed", "Reaktion entfernt"],
  },
  look: {
    icon: "message",
  },
  requires: {
    intents: ["Guilds", "GuildMessageReactions"],
  },
  fields: {
    emoji: { type: "text", pin: false, default: "", description: "Only this emoji (such as 👍 or a custom emoji name). Empty = any." },
    messageId: { type: "text", pin: false, default: "", label: "Message id", description: "Only reactions on this message. Empty = any." },
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Don't run for bots." },
  },
  outputs: {
    user: { type: "discord.user", description: "Who removed the reaction." },
    message: { type: "discord.message", description: "The message." },
    emoji: { type: "text", description: "The emoji as text." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("messageReactionRemove", async (reaction, user) => {
      if (ctx.fields.ignoreBots && user.bot) return;
      if (reaction.partial) {
        try {
          await reaction.fetch();
        } catch {
          return;
        }
      }
      const text = emojiText(reaction.emoji);
      if (ctx.fields.emoji && ctx.fields.emoji !== reaction.emoji.name && ctx.fields.emoji !== text) return;
      if (ctx.fields.messageId && reaction.message.id !== ctx.fields.messageId) return;
      emit({ user, message: reaction.message, emoji: text });
    });
  },
};
