module.exports = {
  id: "discord.create_invite",
  kind: "action",
  info: {
    name: "Create Invite",
    description: "Makes an invite link to a channel.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    tags: ["invite link", "join link"],
  },
  look: {
    icon: "user-plus",
  },
  fields: {
    channel: { type: "discord.channel", pin: "only", required: true, description: "Where the invite leads." },
    hours: { type: "number", default: 24, min: 0, max: 168, description: "How long it works. 0 = forever." },
    uses: { type: "number", default: 0, min: 0, max: 100, step: 1, label: "Max uses", description: "0 = unlimited." },
    temporary: { type: "boolean", default: false, advanced: true, description: "Members who joined with it are removed when they go offline (unless given a role)." },
    unique: { type: "boolean", default: true, advanced: true, description: "Always make a new link instead of reusing a similar one." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  outputs: {
    url: { type: "text", description: "The invite link." },
    code: { type: "text", description: "Its code." },
  },
  async run({ fields: f }) {
    const invite = await f.channel.createInvite({ maxAge: Math.round((f.hours || 0) * 3600), maxUses: Math.floor(f.uses || 0), temporary: Boolean(f.temporary), unique: Boolean(f.unique), reason: f.reason || undefined });
    return { url: invite.url, code: invite.code };
  },
};
