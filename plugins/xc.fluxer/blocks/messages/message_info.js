module.exports = {
  id: "fluxer.message_info",
  kind: "action",
  info: {
    name: "Message Info",
    description: "What a message says, who wrote it and where.",
    author: "XC Team",
    version: "2.0.0",
  },
  search: {
    category: "messages",
    aliases: ["Message content", "Message author"],
  },
  look: {
    icon: "info",
  },
  fields: {
    message: { type: "fluxer.message", pin: "only", required: true, description: "The message." },
  },
  outputs: {
    content: { type: "text", description: "Its text." },
    author: { type: "fluxer.user", description: "Who wrote it." },
    member: { type: "fluxer.member?", description: "Who wrote it, as a server member. Empty in direct messages." },
    channel: { type: "fluxer.channel?", description: "Where it is, when the bot has the channel." },
    server: { type: "fluxer.server?", description: "The server it is in. Empty in direct messages." },
    id: { type: "text", description: "The message's id." },
    sentAt: { type: "date", label: "Sent at", description: "When it was sent." },
    pinned: { type: "boolean", description: "Whether it is pinned." },
    mentions: { type: "list<fluxer.user>", description: "The people mentioned in it." },
    reply: { type: "fluxer.message?", label: "Replies to", description: "The message it answers, when it is a reply." },
  },
  run({ fields }) {
    const m = fields.message;
    if (!m) throw new Error("Connect the message");
    return {
      content: m.content ?? "",
      author: m.author,
      member: m.member ?? null,
      channel: m.channel ?? null,
      server: m.guild ?? null,
      id: m.id,
      sentAt: m.createdAt,
      pinned: Boolean(m.pinned),
      mentions: m.mentions ?? [],
      reply: m.referencedMessage ?? null,
    };
  },
};
