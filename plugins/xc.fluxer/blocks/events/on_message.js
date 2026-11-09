module.exports = {
  id: "fluxer.on_message",
  kind: "event",
  info: {
    name: "Message Sent",
    description: "Runs when a message is sent in a channel the bot can see.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "messages",
    aliases: ["New message", "Message received", "On message"],
  },
  look: {
    icon: "message",
  },
  fields: {
    contains: { type: "text", pin: false, default: "", template: false, description: "Only messages containing this text. Empty = every message." },
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Skip messages sent by bots, including this one." },
  },
  outputs: {
    message: { type: "fluxer.message", description: "The message." },
    author: { type: "fluxer.user", description: "Who sent it." },
    member: { type: "fluxer.member?", description: "Who sent it, as a server member. Empty in direct messages." },
    channel: { type: "fluxer.channel", description: "Where it was sent." },
    server: { type: "fluxer.server?", description: "The server it was sent in. Empty in direct messages." },
    content: { type: "text", description: "The message text." },
  },
  listen(ctx, emit) {
    ctx.fluxer.client.on("messageCreate", (message) => {
      if (ctx.fields.ignoreBots && message.author?.bot) return;
      if (ctx.fields.contains && !String(message.content ?? "").includes(ctx.fields.contains)) return;
      emit({ message, content: message.content ?? "", author: message.author, member: message.member ?? null, channel: message.channel, server: message.guild ?? null });
    });
  },
};
