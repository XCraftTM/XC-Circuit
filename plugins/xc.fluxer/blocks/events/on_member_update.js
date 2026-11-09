module.exports = {
  id: "fluxer.on_member_update",
  kind: "event",
  info: {
    name: "Member Changed",
    description: "Runs when a member's roles, nickname or timeout change in a server. Tells which roles were added and removed.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    aliases: ["On member update", "Roles changed", "Nickname changed", "Member updated"],
  },
  look: {
    icon: "user",
  },
  fields: {},
  outputs: {
    member: { type: "fluxer.member", description: "The member, as they are now." },
    before: { type: "fluxer.member?", description: "The member as they were, when known." },
    server: { type: "fluxer.server?", description: "The server." },
    rolesAdded: { type: "list<text>", label: "Roles added", description: "Ids of roles they got." },
    rolesRemoved: { type: "list<text>", label: "Roles removed", description: "Ids of roles taken away." },
    nicknameBefore: { type: "text", label: "Nickname before", description: "Their nickname before (empty = none or not known)." },
    nickname: { type: "text", description: "Their nickname now (empty = none)." },
  },
  listen(ctx, emit) {
    const ids = (m) => (m?.roles?.roleIds ? [...m.roles.roleIds] : []);
    ctx.fluxer.client.on("guildMemberUpdate", (before, member) => {
      const was = ids(before);
      const now = ids(member);
      emit({
        member,
        before: before ?? null,
        rolesAdded: before ? now.filter((r) => !was.includes(r)) : [],
        rolesRemoved: before ? was.filter((r) => !now.includes(r)) : [],
        nicknameBefore: before?.nickname ?? "",
        nickname: member?.nickname ?? "",
        server: member?.guild ?? null,
      });
    });
  },
};
