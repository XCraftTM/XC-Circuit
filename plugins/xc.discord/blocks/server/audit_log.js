const TYPES = { any: "Anything", MemberKick: "Kicks", MemberBanAdd: "Bans", MemberBanRemove: "Unbans", MemberUpdate: "Member changes (timeouts, nicknames)", MemberRoleUpdate: "Role given or taken", MessageDelete: "Messages deleted by others", MessageBulkDelete: "Bulk deletes", ChannelCreate: "Channels created", ChannelDelete: "Channels deleted", ChannelUpdate: "Channels changed", RoleCreate: "Roles created", RoleDelete: "Roles deleted", RoleUpdate: "Roles changed", InviteCreate: "Invites created", GuildUpdate: "Server settings" };

module.exports = {
  id: "discord.audit_log",
  kind: "action",
  info: {
    name: "Audit Log",
    description: "Reads the server's audit log: who did what (kicks, bans, deletes, role changes …). The bot needs View Audit Log.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    tags: ["who did it", "moderation log", "history"],
  },
  look: {
    icon: "shield",
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    type: { type: "select", pin: false, default: "any", choices: TYPES, description: "Which actions." },
    user: { type: "discord.user?", pin: "only", label: "Done by", description: "Only actions by this person." },
    limit: { type: "number", default: 10, min: 1, max: 100, step: 1, description: "How many entries." },
  },
  outputs: {
    entries: { type: "list<object>", description: "Each entry: action, executorId, targetId, reason, createdAt." },
    executor: { type: "discord.user?", label: "Latest by", description: "Who did the latest one." },
    targetId: { type: "text", label: "Latest target id", description: "Who or what the latest one was done to." },
    reason: { type: "text", label: "Latest reason", description: "The latest one's reason." },
  },
  async run({ fields: f }) {
    const { AuditLogEvent } = require("discord.js");
    const options = { limit: Math.max(1, Math.min(100, Math.floor(f.limit || 10))) };
    if (f.type !== "any") options.type = AuditLogEvent[f.type];
    if (f.user) options.user = f.user.id;
    const log = await f.server.fetchAuditLogs(options);
    const list = [...log.entries.values()];
    const entries = list.map((e) => ({ action: AuditLogEvent[e.action] ?? String(e.action), executorId: e.executorId ?? "", targetId: e.targetId ?? "", reason: e.reason ?? "", createdAt: e.createdAt }));
    const first = list[0];
    return { entries, executor: first?.executor ?? null, targetId: first?.targetId ?? "", reason: first?.reason ?? "" };
  },
};
