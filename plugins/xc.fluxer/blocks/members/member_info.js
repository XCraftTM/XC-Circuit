module.exports = {
  id: "fluxer.member_info",
  kind: "value",
  info: {
    name: "Member Info",
    description: "A member's name in the server, roles, join date and picture.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    aliases: ["Nickname", "Joined at", "Member roles", "Avatar"],
  },
  look: {
    icon: "user",
  },
  fields: {
    member: { type: "fluxer.member", pin: "only", required: true, description: "The member." },
  },
  outputs: {
    name: { type: "text", description: "The name shown in the server: nickname, else display name, else username." },
    nickname: { type: "text", description: "Their nickname in this server. Empty when they have none." },
    user: { type: "fluxer.user", description: "Their account." },
    id: { type: "text", description: "Their id." },
    roles: { type: "list<text>", label: "Role ids", description: "The ids of their roles." },
    joinedAt: { type: "date?", label: "Joined at", description: "When they joined the server." },
    timedOutUntil: { type: "date?", label: "Timed out until", description: "When their timeout ends. Empty when they aren't timed out." },
    avatar: { type: "text", description: "Their picture's web address." },
    server: { type: "fluxer.server", description: "The server." },
    mention: { type: "text", description: "<@id>, which pings them in a message." },
  },
  run({ fields }) {
    const m = fields.member;
    if (!m) throw new Error("Connect the member");
    const until = m.communicationDisabledUntil;
    return {
      name: m.displayName ?? m.nick ?? m.user?.globalName ?? m.user?.username ?? "",
      nickname: m.nick ?? "",
      user: m.user,
      id: m.id,
      roles: [...(m.roles?.roleIds ?? [])],
      joinedAt: m.joinedAt ?? null,
      timedOutUntil: until && new Date(until) > new Date() ? until : null,
      avatar: m.displayAvatarURL?.() ?? m.user?.displayAvatarURL?.() ?? "",
      server: m.guild,
      mention: `<@${m.id}>`,
    };
  },
};
