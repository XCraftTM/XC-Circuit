module.exports = {
  id: "discord.add_role",
  kind: "action",
  info: {
    name: "Add Role",
    description: "Gives a member a role. The bot's own role must sit higher than this role.",
    author: "XC Team",
    version: "2.0.0",
  },
  search: {
    category: "roles",
    aliases: ["Give role", "Assign role"],
    tags: ["rank", "permissions"],
  },
  look: {
    icon: "tag",
  },
  fields: {
    member: { type: "discord.member", pin: "only", required: true, description: "The member who gets the role." },
    roleId: { type: "text", required: true, default: "", label: "Role id", description: "The role to give (right-click a role › Copy Role ID)." },
    reason: { type: "text", advanced: true, default: "", description: "Shown in the server's audit log next to the role change." },
  },
  outputs: {
    member: { type: "discord.member", description: "The same member, now with the role." },
  },
  async run({ fields }) {
    await fields.member.roles.add(fields.roleId?.id ?? fields.roleId, fields.reason || undefined);
    return { member: fields.member };
  },
};
