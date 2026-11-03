module.exports = {
  id: "discord.leave_server",
  kind: "action",
  info: {
    name: "Leave Server",
    description: "Makes the bot leave a server.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    tags: ["leave guild", "quit"],
  },
  look: {
    icon: "layers",
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server to leave." },
  },
  async run({ fields }) {
    await fields.server.leave();
  },
};
