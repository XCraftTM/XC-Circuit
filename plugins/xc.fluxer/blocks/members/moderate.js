const ACTIONS = {
  kick: "Kick",
  ban: "Ban",
  tempban: "Ban for a while",
  unban: "Unban",
  timeout: "Time out",
  untimeout: "End the timeout",
};

module.exports = {
  id: "fluxer.moderate",
  kind: "action",
  info: {
    name: "Kick, Ban or Time Out",
    description: "Kicks, bans (also for a while), unbans or times out someone. The bot needs the permission for it and a higher role than theirs.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["Kick", "Ban", "Unban", "Timeout", "Mute", "Temp ban"],
  },
  look: {
    icon: "shield",
  },
  requires: {
    permissions: {
      action: {
        kick: ["KickMembers"],
        ban: ["BanMembers"],
        tempban: ["BanMembers"],
        unban: ["BanMembers"],
        timeout: ["ModerateMembers"],
        untimeout: ["ModerateMembers"],
      },
    },
  },
  fields: {
    action: { type: "select", pin: false, default: "kick", choices: ACTIONS, description: "What to do." },
    who: { type: "any", pin: true, required: true, description: "A member, a user, or their id or mention (<@id>)." },
    server: { type: "fluxer.server", pin: "only", description: "The server. Needed when Who is a user or an id." },
    reason: { type: "text", default: "", description: "Why, for the server's audit log." },
    minutes: { type: "number", default: 10, min: 1, label: "Minutes", show: { action: ["timeout", "tempban"] }, description: "How long, in minutes." },
    deleteDays: {
      type: "number",
      default: 0,
      min: 0,
      max: 7,
      advanced: true,
      label: "Delete messages (days)",
      show: { action: ["ban", "tempban"] },
      description: "Also delete their messages from this many past days (0 to 7).",
    },
  },
  async run({ fields, ctx }) {
    const reason = String(fields.reason ?? "").trim() || undefined;
    const who = fields.who;
    const isMember = who && typeof who === "object" && who.roles && who.guild;
    const server = fields.server ?? (isMember ? who.guild : null);
    const userId = ctx.fluxer.id(who, "user");
    if (!userId) throw new Error("Connect who, or type their id");
    if (!server) throw new Error("Connect the server too, when Who is a user or an id");
    const minutes = Math.max(1, Number(fields.minutes) || 1);
    const deleteMessageDays = Math.min(7, Math.max(0, Math.round(Number(fields.deleteDays) || 0))) || undefined;
    switch (fields.action) {
      case "ban":
        return void (await server.ban(userId, { reason, deleteMessageDays }));
      case "tempban":
        return void (await server.ban(userId, { reason, deleteMessageDays, banDurationSeconds: Math.round(minutes * 60) }));
      case "unban":
        return void (await server.unban(userId, reason));
      case "timeout":
      case "untimeout": {
        const member = isMember ? who : await server.fetchMember(userId);
        return void (await member.timeout(fields.action === "timeout" ? minutes * 60_000 : null, reason));
      }
      default: {
        const member = isMember ? who : await server.fetchMember(userId);
        await member.kick(reason);
      }
    }
  },
};
