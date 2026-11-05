module.exports = {
  id: "fluxer.get_messages",
  kind: "action",
  info: {
    name: "Get Messages",
    description: "Loads the latest messages in a channel, or one message by its id.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "messages",
    aliases: ["Fetch messages", "Find message", "Message history"],
  },
  look: {
    icon: "message",
  },
  requires: {
    permissions: ["ReadMessageHistory"],
  },
  fields: {
    channel: { type: "fluxer.channel", pin: "only", required: true, description: "The channel." },
    messageId: { type: "text", default: "", label: "Message id", description: "One message by its id. Empty = the latest messages." },
    limit: { type: "number", default: 20, min: 1, max: 100, description: "How many of the latest messages (1 to 100)." },
  },
  outputs: {
    messages: { type: "list<fluxer.message>", description: "The messages, newest first." },
    message: { type: "fluxer.message?", description: "The message with that id, or the newest one." },
  },
  async run({ fields, ctx }) {
    const channel = await ctx.fluxer.channel(fields.channel);
    if (!channel.messages) throw new Error("That channel has no messages (a voice channel or category?)");
    const id = String(fields.messageId ?? "").trim();
    if (id) {
      const message = await channel.messages.fetch(id);
      return { messages: [message], message };
    }
    const limit = Math.min(100, Math.max(1, Math.round(Number(fields.limit) || 20)));
    const found = await channel.messages.fetch({ limit });
    const messages = [...(found?.values?.() ?? found ?? [])];
    return { messages, message: messages[0] ?? null };
  },
};
