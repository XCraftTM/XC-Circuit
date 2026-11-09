module.exports = {
  id: "discord.on_message",
  kind: "event",
  info: {
    name: "Message Sent",
    description: "Runs when a message is sent in a channel the bot can see.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "messages",
    aliases: ["New message", "Message received", "On message"],
  },
  look: {
    icon: "message",
  },
  requires: {
    intents: ["Guilds", "GuildMessages", "MessageContent"],
  },
  fields: {
    contains: { type: "text", pin: false, default: "", description: "Only messages containing this text. Empty = every message." },
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Skip messages sent by bots, including this one." },
  },
  outputs: {
    message: { type: "discord.message", description: "The message." },
    author: { type: "discord.user", description: "Who sent it." },
    member: { type: "discord.member?", description: "Who sent it, as a server member. Empty in DMs." },
    channel: { type: "discord.channel", description: "Where it was sent." },
    server: { type: "discord.guild?", description: "The server it was sent in. Empty in DMs." },
    content: { type: "text", description: "The message text." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("messageCreate", (message) => {
      if (ctx.fields.ignoreBots && message.author.bot) return;
      if (ctx.fields.contains && !message.content.includes(ctx.fields.contains)) return;
      emit({ message, content: message.content, author: message.author, member: message.member ?? null, channel: message.channel, server: message.guild ?? null });
    });
  },
};
