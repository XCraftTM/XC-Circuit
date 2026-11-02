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
  run({ fields }) {
    const [, , read] = INFO[fields.info] || INFO.content;
    return { result: read(fields.message) };
  },
};
