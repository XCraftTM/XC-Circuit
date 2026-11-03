module.exports = {
  id: "discord.prune_members",
  kind: "action",
  info: {
    name: "Prune Members",
    description: "Kicks members who haven't been online for a number of days and have no roles (or only the roles you allow). Test first: it can count without kicking. The bot needs Kick Members and Manage Server.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["Kick inactive", "Remove inactive members", "Member prune"],
    tags: ["prune", "inactive", "kick", "cleanup", "moderation"],
  },
  look: {
    icon: "trash",
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    days: { type: "number", default: 30, min: 1, max: 30, description: "Offline for at least this many days." },
    roles: { type: "list<discord.role>", pin: "only", multi: true, label: "Also with roles", description: "Members with only these roles count too. None = only members without roles." },
    test: { type: "boolean", pin: false, default: true, label: "Only count", description: "Count who would be kicked, without kicking." },
    reason: { type: "text", template: true, default: "", description: "Shown in the audit log." },
  },
  outputs: {
    count: { type: "number", description: "How many were (or would be) kicked." },
  },
  async run({ fields: f }) {
    const s = f.server;
    if (!s?.members?.prune) throw new Error("Connect a server");
    const count = await s.members.prune({
      days: Math.max(1, Math.min(30, Math.floor(Number(f.days) || 30))),
      dry: Boolean(f.test),
      roles: [f.roles].flat().filter(Boolean),
      count: true,
      reason: f.reason || undefined,
    });
    return { count: count ?? 0 };
  },
};
