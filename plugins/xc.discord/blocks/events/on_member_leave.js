module.exports = {
  id: "discord.on_member_leave",
  kind: "event",
  info: {
    name: "Member Leaves",
    description: "Runs when someone leaves (or is kicked from) a server the bot is in.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events",
  },
  look: {
    icon: "user",
  },
  requires: {
    intents: ["Guilds", "GuildMembers"],
  },
  fields: {
    serverId: { type: "text", pin: false, default: "", label: "Server id", description: "Only this server. Empty = all servers." },
  },
  outputs: {
    member: { type: "discord.member", description: "The member who left." },
    server: { type: "discord.guild", description: "The server they left." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("guildMemberRemove", (member) => {
      if (ctx.fields.serverId && member.guild.id !== ctx.fields.serverId) return;
      emit({ member, server: member.guild });
    });
  },
};
