const ACTIONS = { list: "List invites", info: "Look up an invite", delete: "Delete an invite" };
const WHERE = { server: "In the whole server", channel: "In one channel" };

module.exports = {
  id: "fluxer.invites",
  kind: "action",
  info: {
    name: "Invites",
    description: "Lists a server's or channel's invites (with their uses), looks one up by its code, or deletes one. Make new ones with Create Invite.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Invite list", "Delete invite", "Invite uses", "Invite tracker"],
  },
  look: {
    icon: "user-plus",
  },
  requires: {
    permissions: { action: { list: ["ManageGuild"], delete: ["ManageGuild"] } },
  },
  fields: {
    action: { type: "select", pin: false, default: "list", choices: ACTIONS, description: "What to do." },
    where: { type: "select", pin: false, default: "server", choices: WHERE, show: { action: ["list"] }, description: "Which invites to list." },
    server: { type: "fluxer.server", pin: "only", show: { action: ["list", "info", "delete"] }, description: "The server." },
    channel: { type: "fluxer.channel", pin: "only", show: { action: ["list"] }, description: "For one channel: the channel." },
    code: { type: "text", default: "", show: { action: ["info", "delete"] }, description: "The invite's code (or its whole link)." },
  },
  outputs: {
    invites: { type: "list<object>", description: "For lists: each invite's code, uses, maxUses, inviter and expiry." },
    invite: { type: "fluxer.invite?", description: "For look-ups: the invite." },
    uses: { type: "number", description: "For look-ups: how often it was used." },
    inviter: { type: "fluxer.user?", description: "For look-ups: who made it." },
  },
  async run({ fields: f, ctx }) {
    const shape = (i) => ({ code: i.code, uses: i.uses ?? 0, maxUses: i.maxUses ?? 0, inviter: i.inviter ?? null, inviterId: i.inviter?.id ?? null, expiresAt: i.expiresAt ?? null, channelId: i.channelSnapshot?.id ?? null });
    if (f.action === "list") {
      const source = f.where === "channel" ? await ctx.fluxer.channel(f.channel) : await ctx.fluxer.server(f.server ?? f.channel);
      return { invites: (await source.fetchInvites()).map(shape), invite: null, uses: 0, inviter: null };
    }
    const code = String(f.code ?? "").trim().split("/").pop();
    if (!code) throw new Error("Give the invite's code");
    const server = await ctx.fluxer.server(f.server);
    const invite = await server.fetchInvite(code);
    if (!invite) throw new Error(`No invite ${code}`);
    if (f.action === "delete") {
      await invite.delete();
      return { invites: [], invite: null, uses: 0, inviter: null };
    }
    return { invites: [], invite, uses: invite.uses ?? 0, inviter: invite.inviter ?? null };
  },
};
