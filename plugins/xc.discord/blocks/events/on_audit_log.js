module.exports = {
  id: "discord.on_audit_log",
  kind: "event",
  info: {
    name: "Audit Log Entry",
    description: "Runs when something is written to a server's audit log: who did what to whom, and why. The best start for a mod log. The bot needs View Audit Log.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_server",
    aliases: ["Mod log", "Audit log created", "Who did it"],
    tags: ["audit", "log", "moderation", "kick", "ban", "executor"],
  },
  look: {
    icon: "shield",
  },
  requires: {
    intents: ["Guilds", "GuildModeration"],
  },
  fields: {
    action: {
      type: "text",
      pin: false,
      default: "",
      placeholder: "MemberKick",
      description: "Only this kind, as discord.js names it: MemberKick, MemberBanAdd, MemberRoleUpdate, ChannelCreate, MessageDelete … Empty = every entry.",
    },
  },
  outputs: {
    action: { type: "text", description: "What happened, such as MemberKick." },
    executor: { type: "discord.user?", description: "Who did it." },
    target: { type: "object", description: "What it was done to (a user, channel, role …), when known." },
    targetId: { type: "text", label: "Target id", description: "The id of what it was done to." },
    reason: { type: "text", description: "The reason given, if any." },
    changes: { type: "list<object>", description: "What changed: each key with its old and new value." },
    entry: { type: "object", description: "The whole audit log entry." },
    server: { type: "discord.guild", description: "The server." },
  },
  listen(ctx, emit) {
    const { AuditLogEvent } = ctx.require("discord.js");
    ctx.discord.client.on("guildAuditLogEntryCreate", async (entry, server) => {
      const action = AuditLogEvent[entry.action] ?? String(entry.action);
      if (ctx.fields.action && action.toLowerCase() !== String(ctx.fields.action).trim().toLowerCase()) return;
      let executor = entry.executor ?? null;
      if (!executor && entry.executorId) executor = await ctx.discord.client.users.fetch(entry.executorId).catch(() => null);
      emit({
        action,
        executor,
        target: entry.target ?? null,
        targetId: entry.targetId ?? "",
        reason: entry.reason ?? "",
        changes: (entry.changes || []).map((c) => ({ key: c.key, old: c.old ?? null, new: c.new ?? null })),
        entry,
        server,
      });
    });
  },
};
