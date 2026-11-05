module.exports = {
  id: "fluxer.create_invite",
  kind: "action",
  info: {
    name: "Create Invite",
    description: "Makes an invite link to a channel's server.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["Invite link", "Invite"],
  },
  look: {
    icon: "user-plus",
  },
  requires: {
    permissions: ["CreateInstantInvite"],
  },
  fields: {
    channel: { type: "fluxer.channel", pin: "only", required: true, description: "The channel people land in." },
    hours: { type: "number", default: 24, min: 0, description: "How long it works, in hours. 0 = forever." },
    uses: { type: "number", default: 0, min: 0, description: "How many people can use it. 0 = no limit." },
  },
  outputs: {
    url: { type: "text", label: "Link", description: "The invite link." },
    code: { type: "text", description: "The invite's code." },
  },
  async run({ fields, ctx }) {
    const channel = await ctx.fluxer.channel(fields.channel);
    if (typeof channel.createInvite !== "function") throw new Error("Invites can only be made for server channels");
    const invite = await channel.createInvite({ maxAge: Math.round((Number(fields.hours) || 0) * 3600), maxUses: Math.round(Number(fields.uses) || 0) });
    return { url: invite.url, code: invite.code };
  },
};
