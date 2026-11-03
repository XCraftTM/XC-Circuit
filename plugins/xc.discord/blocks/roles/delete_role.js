module.exports = {
  id: "discord.delete_role",
  kind: "action",
  info: {
    name: "Delete Role",
    description: "Deletes a role from its server. The bot needs Manage Roles and a higher role.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "roles",
    aliases: ["Remove role"],
    tags: ["remove role"],
  },
  look: {
    icon: "trash",
  },
  fields: {
    role: { type: "discord.role", pin: "only", required: true, description: "The role to delete." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  async run({ fields }) {
    await fields.role.delete(fields.reason || undefined);
  },
};
