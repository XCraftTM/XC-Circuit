module.exports = {
  id: "discord.on_reaction",
  name: "Reaction Added",
  description: "Runs when someone reacts to a message. Good for reaction roles.",
  author: "XC Team",
  version: "1.0.0",
  category: "events",
  icon: "message",
  kind: "event",
  intents: ["Guilds", "GuildMessageReactions"],
  fields: {
    emoji: { type: "text", pin: false, default: "", description: "Only this emoji (such as 👍 or a custom emoji name). Empty = any." },
    messageId: { type: "text", pin: false, default: "", label: "Message id", description: "Only reactions on this message. Empty = any." },
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Don't run when a bot adds the reaction." },
  },
  outputs: {
    user: { type: "discord.user", description: "Who reacted." },
    message: { type: "discord.message", description: "The message." },
    emoji: { type: "text", description: "The emoji as text." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("messageReactionAdd", async (reaction, user) => {
      if (ctx.fields.ignoreBots && user.bot) return;
      if (reaction.partial) {
        try {
          await reaction.fetch();
        } catch {
          return;
        }
      }
      const e = reaction.emoji;
      const text = e.id ? `<${e.animated ? "a" : ""}:${e.name}:${e.id}>` : e.name;
      if (ctx.fields.emoji && ctx.fields.emoji !== e.name && ctx.fields.emoji !== text) return;
      if (ctx.fields.messageId && reaction.message.id !== ctx.fields.messageId) return;
      emit({ user, message: reaction.message, emoji: text });
    });
  },
};
