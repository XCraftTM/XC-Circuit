module.exports = {
  id: "fluxer.audit_log",
  kind: "action",
  info: {
    name: "Audit Log",
    description: "Gets the newest entries of a server's audit log: who did what, and why. For mod logs and finding out who kicked or banned someone.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["Mod log", "Who banned", "Who kicked"],
  },
  look: {
    icon: "shield",
  },
  requires: {
    permissions: ["ViewAuditLog"],
  },
  fields: {
    server: { type: "fluxer.server", pin: "only", required: true, description: "The server." },
    by: { type: "any", pin: true, description: "Only entries by this person (a user, member or id). Empty = anyone." },
    limit: { type: "number", default: 10, min: 1, max: 100, step: 1, description: "How many entries (up to 100)." },
  },
  outputs: {
    entries: { type: "list<object>", description: "The entries, newest first: action, userId, targetId, reason and changes." },
    newest: { type: "object?", description: "The newest entry." },
  },
  async run({ fields: f, ctx }) {
    if (!f.server) throw new Error("Connect the server");
    const userId = f.by ? ctx.fluxer.id(f.by, "user") : undefined;
    const payload = await f.server.fetchAuditLogs({ limit: Math.min(100, Math.max(1, Math.floor(f.limit || 10))), userId: userId || undefined });
    const entries = (payload?.entries || []).map((e) => ({
      id: e.id,
      action: Number(e.action_type ?? e.actionType ?? 0),
      userId: String(e.user_id ?? e.userId ?? ""),
      targetId: String(e.target_id ?? e.targetId ?? ""),
      reason: String(e.reason ?? ""),
      changes: e.changes ?? [],
    }));
    return { entries, newest: entries[0] ?? null };
  },
};
