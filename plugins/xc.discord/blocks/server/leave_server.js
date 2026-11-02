module.exports = {
  id: "discord.leave_server",
  name: "Leave Server",
  description: "Makes the bot leave a server.",
  author: "XC Team",
  version: "1.0.0",
  category: "server",
  icon: "layers",
  kind: "action",
  tags: ["leave guild", "quit"],
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server to leave." },
  },
  async run({ fields }) {
    await fields.server.leave();
  },
};
