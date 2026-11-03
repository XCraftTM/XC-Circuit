module.exports = {
  id: "discord.has_role",
  kind: "value",
  info: {
    name: "Has Role",
    description: "Whether a member has a role (by id or name).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    tags: ["role", "check", "permission"],
  },
  look: {
    icon: "tag",
  },
  fields: {
    member: { type: "discord.member", pin: "only", required: true, description: "The member whose roles are checked." },
    role: { type: "text", required: true, default: "", description: "A role id or exact role name." },
  },
  outputs: {
    result: { type: "boolean", description: "True when the member has the role." },
  },
  run({ fields }) {
    const r = String(fields.role?.id ?? fields.role);
    return { result: fields.member.roles.cache.some((role) => role.id === r || role.name === r) };
  },
};
