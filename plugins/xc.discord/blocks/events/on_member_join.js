module.exports = {
  id: "discord.on_member_join",
  name: "Member Joins",
  description: "Runs when someone joins a server the bot is in.",
  author: "XC Team",
  version: "1.0.0",
  category: "events",
  icon: "user-plus",
  kind: "event",
  intents: ["Guilds", "GuildMembers"],
  fields: {
    serverId: { type: "text", pin: false, default: "", label: "Server id", description: "Only this server. Empty = all servers." },
  },
  outputs: {
    member: { type: "discord.member", description: "The member who joined." },
    server: { type: "discord.guild", description: "The server they joined." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("guildMemberAdd", (member) => {
      if (ctx.fields.serverId && member.guild.id !== ctx.fields.serverId) return;
      emit({ member, server: member.guild });
    });
  },
};
