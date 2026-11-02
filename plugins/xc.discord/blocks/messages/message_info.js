const INFO = {
  content: ["Text", "text", (m) => m.content],
  id: ["Id", "text", (m) => m.id],
  author: ["Author", "discord.user", (m) => m.author],
  member: ["Author as member", "discord.member?", (m) => m.member],
  channel: ["Channel", "discord.channel", (m) => m.channel],
  url: ["Link", "text", (m) => m.url],
  createdAt: ["Sent at", "date", (m) => m.createdAt],
  mentions: ["Mentioned users", "list<discord.user>", (m) => [...m.mentions.users.values()]],
  attachments: ["Attachment URLs", "list<text>", (m) => [...m.attachments.values()].map((a) => a.url)],
  isBot: ["Sent by a bot", "boolean", (m) => m.author.bot],
  server: ["Server", "discord.guild?", (m) => m.guild],
  editedAt: ["Edited at", "date", (m) => m.editedAt],
  pinned: ["Is pinned", "boolean", (m) => m.pinned],
  isReply: ["Is a reply", "boolean", (m) => Boolean(m.reference?.messageId)],
  replyTo: ["Replied-to message", "discord.message?", (m) => (m.reference?.messageId ? m.fetchReference().catch(() => null) : null)],
  roleMentions: ["Mentioned roles", "list<discord.role>", (m) => [...m.mentions.roles.values()]],
  channelMentions: ["Mentioned channels", "list<discord.channel>", (m) => [...m.mentions.channels.values()]],
  mentionsEveryone: ["Mentions @everyone", "boolean", (m) => m.mentions.everyone],
  embeds: ["Embeds", "list<object>", (m) => m.embeds.map((e) => e.toJSON())],
  reactions: ["Reactions", "list<object>", (m) => [...m.reactions.cache.values()].map((r) => ({ emoji: r.emoji.toString(), count: r.count }))],
  stickers: ["Sticker names", "list<text>", (m) => [...m.stickers.values()].map((s) => s.name)],
  thread: ["Its thread", "discord.channel?", (m) => m.thread],
};

module.exports = {
  id: "discord.message_info",
  name: "Message Info",
  description: "Reads one piece of information about a message.",
  author: "XC Team",
  version: "1.0.0",
  category: "messages",
  icon: "message",
  kind: "value",
  fields: {
    message: { type: "discord.message", pin: "only", required: true, description: "The message to read from." },
    info: { type: "select", pin: false, default: "content", choices: Object.fromEntries(Object.entries(INFO).map(([k, [l]]) => [k, l])), description: "Which detail about the message to get." },
  },
  outputs: {
    result: { type: "text", description: "The value you picked." },
  },
  define({ values }) {
    const [label, type] = INFO[values.info] || INFO.content;
    return { outputs: { result: { type, label, description: `The message's ${label.toLowerCase()}.` } } };
  },
  async run({ fields }) {
    const [, , read] = INFO[fields.info] || INFO.content;
    return { result: await read(fields.message) };
  },
};
