const ACTIONS = {
  timeout: "Time out",
  untimeout: "Remove time out",
  kick: "Kick",
  ban: "Ban",
  unban: "Unban (by user id)",
  banId: "Ban by user id (also people not in the server)",
  removeRole: "Remove role",
  nickname: "Change nickname",
};

module.exports = {
  id: "discord.moderate",
  kind: "action",
  info: {
    name: "Moderate Member",
    description: "Times out, kicks, bans or unbans a member, removes a role or changes a nickname. The bot needs the matching permission.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    tags: ["kick", "ban", "mute", "timeout", "role", "nickname"],
  },
  look: {
    icon: "shield",
  },
  fields: {
    action: { type: "select", pin: false, default: "timeout", choices: ACTIONS, description: "What happens to the member: time out, kick, ban, unban or remove a role." },
    member: { type: "discord.member", pin: "only", required: true, description: "Who to moderate.", show: { action: ["timeout", "untimeout", "kick", "ban", "removeRole", "nickname"] } },
    server: { type: "discord.guild", pin: "only", description: "The server to ban or unban in.", show: { action: ["unban", "banId"] } },
    userId: { type: "text", default: "", label: "User id", show: { action: ["unban", "banId"] }, description: "The user's id (banned people aren't members)." },
    minutes: { type: "number", default: 10, min: 1, description: "How long the time out lasts.", show: { action: ["timeout"] } },
    roleId: { type: "text", default: "", label: "Role id", show: { action: ["removeRole"] }, description: "The id of the role to take away." },
    nickname: { type: "text", template: true, default: "", description: "Empty resets it.", show: { action: ["nickname"] } },
    deleteDays: { type: "number", default: 0, min: 0, max: 7, label: "Delete messages (days)", show: { action: ["ban", "banId"] }, description: "Also delete their messages from this many past days (0 to 7)." },
    reason: { type: "text", template: true, default: "", description: "Shown in the audit log." },
  },
  async run({ fields: f }) {
    const reason = f.reason || undefined;
    switch (f.action) {
      case "timeout":
        return void (await f.member.timeout(f.minutes * 60000, reason));
      case "untimeout":
        return void (await f.member.timeout(null, reason));
      case "kick":
        return void (await f.member.kick(reason));
      case "ban":
        return void (await f.member.ban({ reason, deleteMessageSeconds: Math.round((f.deleteDays || 0) * 86400) }));
      case "banId":
        return void (await f.server.members.ban(String(f.userId?.id ?? f.userId), { reason, deleteMessageSeconds: Math.round((f.deleteDays || 0) * 86400) }));
      case "unban":
        return void (await f.server.members.unban(String(f.userId), reason));
      case "removeRole":
        return void (await f.member.roles.remove(f.roleId?.id ?? f.roleId, reason));
      case "nickname":
        return void (await f.member.setNickname(f.nickname || null, reason));
    }
  },
};
