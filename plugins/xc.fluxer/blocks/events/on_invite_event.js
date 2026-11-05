const WHEN = { create: "Created", delete: "Deleted" };

module.exports = {
  id: "fluxer.on_invite_event",
  kind: "event",
  info: {
    name: "Invite Created or Deleted",
    description: "Runs when someone makes an invite link to a server, or one is deleted.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["On invite", "Invite created", "Invite log"],
  },
  look: {
    icon: "user-plus",
  },
  fields: {
    when: { type: "select", pin: false, default: "create", choices: WHEN, label: "When", description: "New or deleted invites." },
  },
  outputs: {
    invite: { type: "fluxer.invite?", description: "The new invite. Empty for deleted ones." },
    code: { type: "text", description: "The invite's code." },
    inviter: { type: "fluxer.user?", description: "Who made it (for new invites)." },
    channelId: { type: "text", label: "Channel id", description: "The channel it leads to." },
    serverId: { type: "text", label: "Server id", description: "The server." },
  },
  listen(ctx, emit) {
    const client = ctx.fluxer.client;
    if (ctx.fields.when === "delete") {
      client.on("inviteDelete", (p) => emit({ invite: null, code: String(p?.code ?? ""), inviter: null, channelId: String(p?.channelId ?? p?.channel_id ?? ""), serverId: String(p?.guildId ?? p?.guild_id ?? "") }));
    } else {
      client.on("inviteCreate", (invite) =>
        emit({ invite, code: invite.code, inviter: invite.inviter ?? null, channelId: String(invite.channelSnapshot?.id ?? ""), serverId: String(invite.guildSnapshot?.id ?? "") }),
      );
    }
  },
};
