module.exports = {
  id: "discord.show_typing",
  name: "Show Typing",
  description: "Shows “Bot is typing …” in a channel for about 10 seconds, or until the bot sends a message.",
  author: "XC Team",
  version: "1.0.0",
  category: "messages",
  icon: "message",
  kind: "action",
  tags: ["typing indicator", "busy"],
  fields: {
    channel: { type: "discord.channel", pin: "only", required: true, description: "Where." },
  },
  async run({ fields }) {
    await fields.channel.sendTyping();
  },
};
