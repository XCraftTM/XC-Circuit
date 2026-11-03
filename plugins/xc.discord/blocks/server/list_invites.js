module.exports = {
  id: "discord.list_invites",
  kind: "action",
  info: {
    name: "List Invites",
    description: "A server's invite links with their uses and who made them. For invite tracking. The bot needs Manage Server.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    tags: ["invite tracking", "uses"],
  },
  look: {
    icon: "user-plus",
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
  },
  outputs: {
    invites: { type: "list<object>", description: "Each invite: code, url, uses, inviterId, channelId." },
    count: { type: "number", description: "How many." },
  },
  async run({ fields }) {
    const all = await fields.server.invites.fetch();
    const invites = [...all.values()].map((i) => ({ code: i.code, url: i.url, uses: i.uses ?? 0, inviterId: i.inviterId ?? "", channelId: i.channelId ?? "" }));
    return { invites, count: invites.length };
  },
};
