module.exports = {
  id: "discord.on_typing",
  kind: "event",
  info: {
    name: "Typing Started",
    description: "Runs when someone starts typing in a channel the bot can see, or in a DM with the bot.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_messages",
    aliases: ["Is typing", "Typing indicator", "Starts typing"],
    tags: ["typing", "writing"],
  },
  look: {
    icon: "keyboard",
  },
  requires: {
    intents: ["Guilds", "GuildMessageTyping", "DirectMessageTyping"],
  },
  fields: {
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Don't run for bots." },
  },
  outputs: {
    user: { type: "discord.user", description: "Who is typing." },
    member: { type: "discord.member?", description: "Who is typing, as a server member. Empty in DMs." },
    channel: { type: "discord.channel", description: "Where." },
    server: { type: "discord.guild?", description: "The server. Empty in DMs." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("typingStart", (typing) => {
      if (ctx.fields.ignoreBots && typing.user.bot) return;
      emit({ user: typing.user, member: typing.member ?? null, channel: typing.channel, server: typing.guild ?? null });
    });
  },
};
