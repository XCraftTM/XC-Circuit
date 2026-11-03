module.exports = {
  id: "discord.show_typing",
  kind: "action",
  info: {
    name: "Show Typing",
    description: "Shows “Bot is typing …” in a channel for about 10 seconds, or until the bot sends a message.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "send",
    aliases: ["Typing indicator", "Tippen anzeigen"],
    tags: ["typing indicator", "busy"],
  },
  look: {
    icon: "message",
  },
  fields: {
    channel: { type: "discord.channel", pin: "only", required: true, description: "Where." },
  },
  async run({ fields }) {
    await fields.channel.sendTyping();
  },
};
