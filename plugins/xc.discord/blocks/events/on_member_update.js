const CHANGES = { any: "Any change", roles: "Roles", nickname: "Nickname", timeout: "Time out", boost: "Boosting" };

module.exports = {
  id: "discord.on_member_update",
  kind: "event",
  info: {
    name: "Member Updated",
    description: "Runs when a member's roles, nickname, time out or boosting change.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_members",
    aliases: ["Role changed", "Nickname changed", "Mitglied geändert"],
    tags: ["log", "role added", "nickname", "boost", "timeout"],
  },
  look: {
    icon: "user",
  },
  requires: {
    intents: ["Guilds", "GuildMembers"],
  },
  fields: {
    change: { type: "select", pin: false, default: "any", choices: CHANGES, description: "Which change runs it." },
  },
  outputs: {
    member: { type: "discord.member", description: "The member, as they are now." },
    rolesAdded: { type: "list<discord.role>", label: "Roles added", description: "Roles they got." },
    rolesRemoved: { type: "list<discord.role>", label: "Roles removed", description: "Roles they lost." },
    oldNickname: { type: "text", label: "Old nickname", description: "The nickname before (empty = none)." },
    nickname: { type: "text", description: "The nickname now (empty = none)." },
    timedOut: { type: "boolean", label: "Timed out", description: "Whether they are timed out now." },
    boosting: { type: "boolean", description: "Whether they boost the server now." },
    server: { type: "discord.guild", description: "The server." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("guildMemberUpdate", (old, now) => {
      if (old.partial) return;
      const added = [...now.roles.cache.filter((r) => !old.roles.cache.has(r.id)).values()];
      const removed = [...old.roles.cache.filter((r) => !now.roles.cache.has(r.id)).values()];
      const changed = {
        roles: added.length + removed.length > 0,
        nickname: old.nickname !== now.nickname,
        timeout: old.isCommunicationDisabled() !== now.isCommunicationDisabled(),
        boost: Boolean(old.premiumSince) !== Boolean(now.premiumSince),
      };
      const which = ctx.fields.change;
      if (which === "any" ? !Object.values(changed).some(Boolean) : !changed[which]) return;
      emit({
        member: now,
        rolesAdded: added,
        rolesRemoved: removed,
        oldNickname: old.nickname ?? "",
        nickname: now.nickname ?? "",
        timedOut: now.isCommunicationDisabled(),
        boosting: Boolean(now.premiumSince),
        server: now.guild,
      });
    });
  },
};
