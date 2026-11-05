module.exports = {
  id: "fluxer.on_audit_log",
  kind: "event",
  info: {
    name: "Audit Log Entry",
    description: "Runs for every new entry in a server's audit log: who did what (kicks, bans, role and channel changes …). The bot needs View Audit Log.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["On audit log", "Mod log"],
  },
  look: {
    icon: "shield",
  },
  requires: {
    permissions: ["ViewAuditLog"],
  },
  fields: {},
  outputs: {
    entry: { type: "object", description: "The whole entry." },
    action: { type: "number", description: "What happened, as Fluxer's action number." },
    userId: { type: "text", label: "User id", description: "Who did it." },
    targetId: { type: "text", label: "Target id", description: "What it was done to: a user, role, channel …" },
    reason: { type: "text", description: "The reason given, if any." },
  },
  listen(ctx, emit) {
    ctx.fluxer.client.on("guildAuditLogEntryCreate", (e) =>
      emit({
        entry: e,
        action: Number(e?.action_type ?? e?.actionType ?? 0),
        userId: String(e?.user_id ?? e?.userId ?? ""),
        targetId: String(e?.target_id ?? e?.targetId ?? ""),
        reason: String(e?.reason ?? ""),
      }),
    );
  },
};
