module.exports = {
  id: "discord.publish_message",
  kind: "action",
  info: {
    name: "Publish Message",
    description: "Publishes a message in an announcement channel, so servers following it get it too.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "send",
    aliases: ["Crosspost", "Announcement", "Nachricht veröffentlichen"],
    tags: ["crosspost", "announcement", "follow"],
  },
  look: {
    icon: "send",
  },
  fields: {
    message: { type: "discord.message", pin: "only", required: true, description: "A message in an announcement channel." },
  },
  async run({ fields }) {
    await fields.message.crosspost();
  },
};
