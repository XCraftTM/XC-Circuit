const KINDS = { all: "All reactions removed", emoji: "One emoji's reactions removed" };

module.exports = {
  id: "discord.on_reactions_cleared",
  kind: "event",
  info: {
    name: "Reactions Cleared",
    description: "Runs when every reaction, or every reaction with one emoji, is removed from a message at once (usually by a moderator).",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "reactions",
    aliases: ["All reactions removed", "Reactions reset"],
    tags: ["reaction", "clear", "remove all"],
  },
  look: {
    icon: "trash",
  },
  requires: {
    intents: ["Guilds", "GuildMessageReactions"],
  },
  fields: {
    kind: { type: "select", pin: false, default: "all", label: "When", choices: KINDS, description: "Which event runs it." },
  },
  outputs: {
    message: { type: "discord.message", description: "The message." },
    channel: { type: "discord.channel", description: "Where the message is." },
    server: { type: "discord.guild?", description: "The server it happened in. Empty in DMs." },
    emoji: { type: "text", description: "For one emoji: the emoji as text." },
    count: { type: "number", description: "How many different emojis were removed." },
  },
  listen(ctx, emit) {
    const c = ctx.discord.client;
    if (ctx.fields.kind === "emoji") c.on("messageReactionRemoveEmoji", (reaction) => emit({ message: reaction.message, emoji: reaction.emoji.toString(), count: 1, channel: reaction.message.channel, server: reaction.message.guild ?? null }));
    else c.on("messageReactionRemoveAll", (message, reactions) => emit({ message, emoji: "", count: reactions.size, channel: message.channel, server: message.guild ?? null }));
  },
};
