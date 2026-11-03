module.exports = {
  id: "discord.remove_role",
  kind: "action",
  info: {
    name: "Remove Role",
    description: "Takes a role away from a member. The bot's own role must sit higher than this role.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "roles",
    aliases: ["Take role"],
    tags: ["take role", "unrank"],
  },
  look: {
    icon: "tag",
  },
  fields: {
    member: { type: "discord.member", pin: "only", required: true, description: "The member who loses the role." },
    role: { type: "any", required: true, default: "", label: "Role", description: "A role, or its id." },
    reason: { type: "text", advanced: true, default: "", description: "Shown in the audit log." },
  },
  outputs: {
    member: { type: "discord.member", description: "The same member, without the role." },
  },
  async run({ fields }) {
    await fields.member.roles.remove(fields.role?.id ?? String(fields.role), fields.reason || undefined);
    return { member: fields.member };
  },
};
