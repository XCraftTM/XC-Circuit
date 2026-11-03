module.exports = {
  id: "discord.delete_invite",
  kind: "action",
  info: {
    name: "Delete Invite",
    description: "Deletes an invite by its code or link, so it stops working. The bot needs Manage Server (or Manage Channels for the invite's channel).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Revoke invite", "Remove invite link"],
    tags: ["invite", "delete", "revoke", "link"],
  },
  look: {
    icon: "trash",
  },
  fields: {
    code: { type: "text", default: "", label: "Invite", description: "The invite's code or link, such as discord.gg/abc123." },
    reason: { type: "text", template: true, default: "", description: "Shown in the audit log." },
  },
  outputs: {
    code: { type: "text", description: "The deleted invite's code." },
    uses: { type: "number", description: "How often it had been used, when known." },
  },
  async run({ fields: f, ctx }) {
    const code = String(f.code || "").trim().replace(/^.*\//, "");
    if (!code) throw new Error("Give the invite's code or link");
    const invite = await ctx.discord.client.fetchInvite(code);
    await invite.delete(f.reason || undefined);
    return { code: invite.code, uses: invite.uses ?? 0 };
  },
};
