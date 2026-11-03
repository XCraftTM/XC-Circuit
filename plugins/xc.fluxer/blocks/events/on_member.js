const WHEN = { join: "Joins", leave: "Leaves" };

module.exports = {
  id: "fluxer.on_member",
  kind: "event",
  info: {
    name: "Member Joins or Leaves",
    description: "Runs when someone joins a server, or leaves it (also when kicked or banned).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    aliases: ["Member joined", "Member left", "Welcome", "Goodbye", "On join", "On leave"],
  },
  look: {
    icon: "user-plus",
  },
  fields: {
    when: { type: "select", pin: false, default: "join", choices: WHEN, label: "When", description: "Joining or leaving." },
    ignoreBots: { type: "boolean", pin: false, default: false, label: "Ignore bots", description: "Skip bots joining or leaving." },
  },
  outputs: {
    member: { type: "fluxer.member", description: "Who joined or left. After leaving, only their user and server are certain." },
    user: { type: "fluxer.user", description: "Their account." },
    server: { type: "fluxer.server", description: "The server." },
  },
  listen(ctx, emit) {
    const event = ctx.fields.when === "leave" ? "guildMemberRemove" : "guildMemberAdd";
    ctx.fluxer.client.on(event, (member) => {
      if (ctx.fields.ignoreBots && member.user?.bot) return;
      emit({ member, user: member.user, server: member.guild });
    });
  },
};
