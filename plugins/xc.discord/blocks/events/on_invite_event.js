const KINDS = { created: "Created", deleted: "Deleted" };

module.exports = {
  id: "discord.on_invite_event",
  name: "Invite Created / Deleted",
  description: "Runs when an invite link is made or deleted.",
  author: "XC Team",
  version: "1.0.0",
  category: "events",
  icon: "user-plus",
  kind: "event",
  intents: ["Guilds", "GuildInvites"],
  tags: ["invite tracking", "log"],
  fields: {
    kind: { type: "select", pin: false, default: "created", label: "When", choices: KINDS, description: "Created or deleted." },
  },
  outputs: {
    code: { type: "text", description: "The invite's code." },
    url: { type: "text", description: "The invite link." },
    inviter: { type: "discord.user?", description: "Who made it (for new invites)." },
    channel: { type: "discord.channel?", description: "The channel it leads to." },
    server: { type: "discord.guild?", description: "The server." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on(ctx.fields.kind === "deleted" ? "inviteDelete" : "inviteCreate", (i) =>
      emit({ code: i.code, url: i.url, inviter: i.inviter ?? null, channel: i.channel ?? null, server: i.guild ?? null }),
    );
  },
};
