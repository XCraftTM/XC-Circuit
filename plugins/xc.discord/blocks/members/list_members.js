module.exports = {
  id: "discord.list_members",
  kind: "action",
  info: {
    name: "List Members",
    description: "The members of a server, or only those with a role. Needs the Server Members intent.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    tags: ["everyone", "all members", "role members", "count"],
  },
  look: {
    icon: "user",
  },
  requires: {
    intents: ["Guilds", "GuildMembers"],
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    role: { type: "discord.role?", pin: "only", description: "Only members with this role. Empty = everyone." },
    bots: { type: "boolean", default: false, label: "Include bots", description: "Also list bots." },
    limit: { type: "number", default: 1000, min: 1, step: 1, advanced: true, description: "At most this many." },
  },
  outputs: {
    members: { type: "list<discord.member>", description: "The members." },
    count: { type: "number", description: "How many." },
  },
  async run({ fields: f }) {
    const all = await f.server.members.fetch();
    const list = [...all.values()].filter((m) => (f.bots || !m.user.bot) && (!f.role || m.roles.cache.has(f.role.id))).slice(0, Math.max(1, Math.floor(f.limit || 1000)));
    return { members: list, count: list.length };
  },
};
