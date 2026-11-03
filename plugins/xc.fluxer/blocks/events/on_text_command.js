module.exports = {
  id: "fluxer.on_text_command",
  kind: "event",
  info: {
    name: "Text Command",
    description: "Runs when someone sends a command such as !ping or !ban @someone spam. The prefix is set in Bot › Settings.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "commands",
    aliases: ["Command", "Prefix command", "On command"],
  },
  look: {
    icon: "slash",
  },
  fields: {
    command: {
      type: "text",
      pin: false,
      default: "ping",
      template: false,
      description: "The command without the prefix. Several names for the same command: separate them with commas (ban, b).",
    },
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Skip commands sent by bots, including this one." },
  },
  outputs: {
    message: { type: "fluxer.message", description: "The message with the command." },
    args: { type: "list<text>", label: "Arguments", description: "The words after the command: !ban @a spam gives [@a, spam]." },
    rest: { type: "text", label: "Text after", description: "Everything after the command, as one text." },
    first: { type: "text", label: "First argument", description: "The first word after the command, such as the @someone in !kick @someone being rude." },
    more: { type: "text", label: "Text after the first", description: "Everything after the first argument, such as the reason in !kick @someone being rude." },
    author: { type: "fluxer.user", description: "Who used the command." },
    member: { type: "fluxer.member?", description: "Who used it, as a server member. Empty in direct messages." },
    channel: { type: "fluxer.channel", description: "Where it was used." },
    server: { type: "fluxer.server?", description: "The server it was used in. Empty in direct messages." },
  },
  listen(ctx, emit) {
    const names = String(ctx.fields.command || "")
      .split(",")
      .map((n) => n.trim().replace(/^[!./?$-]/, "").toLowerCase())
      .filter(Boolean);
    ctx.fluxer.client.on("messageCreate", (message) => {
      if (ctx.fields.ignoreBots && message.author?.bot) return;
      const used = ctx.fluxer.parseCommand(message.content, ctx.fluxer.prefix());
      if (!used || !names.includes(used.command)) return;
      const first = used.args[0] ?? "";
      const more = used.rest.slice(first.length).trim();
      emit({ message, args: used.args, rest: used.rest, first, more, author: message.author, member: message.member ?? null, channel: message.channel, server: message.guild ?? null });
    });
  },
};
