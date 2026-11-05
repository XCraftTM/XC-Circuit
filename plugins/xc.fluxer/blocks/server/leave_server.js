module.exports = {
  id: "fluxer.leave_server",
  kind: "action",
  info: {
    name: "Leave Server",
    description: "Makes the bot leave a server. It can only come back with a new invite.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Leave guild", "Bot leave"],
  },
  look: {
    icon: "x",
  },
  fields: {
    server: { type: "any", pin: true, required: true, description: "The server (or its id)." },
  },
  async run({ fields: f, ctx }) {
    const serverId = ctx.fluxer.id(f.server);
    if (!serverId) throw new Error("Connect the server, or type its id");
    await ctx.fluxer.client.user.leaveGuild(serverId);
  },
};
