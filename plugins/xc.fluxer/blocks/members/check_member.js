// The permissions people check most, by fluxer.js name.
const PERMISSIONS = {
  Administrator: "Administrator",
  ManageGuild: "Manage server",
  ManageRoles: "Manage roles",
  ManageChannels: "Manage channels",
  ManageMessages: "Manage messages",
  KickMembers: "Kick members",
  BanMembers: "Ban members",
  ModerateMembers: "Time out members",
  ManageNicknames: "Manage nicknames",
  MentionEveryone: "Mention @everyone",
  ViewAuditLog: "View the audit log",
  SendMessages: "Send messages",
};
const CHECKS = { role: "Has a role", permission: "Has a permission" };

module.exports = {
  id: "fluxer.check_member",
  kind: "value",
  info: {
    name: "Member Has",
    description: "Whether a member has a role, or a permission such as Ban members. For commands only some may use.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "roles",
    aliases: ["Has role", "Has permission", "Is admin", "Check permission"],
  },
  look: {
    icon: "key",
  },
  fields: {
    member: { type: "fluxer.member", pin: "only", required: true, description: "Who." },
    check: { type: "select", pin: false, default: "role", choices: CHECKS, description: "What to check." },
    role: { type: "any", default: "", show: { check: ["role"] }, description: "The role: connect one, or type its id or <@&id>." },
    permission: { type: "select", pin: false, default: "Administrator", choices: PERMISSIONS, show: { check: ["permission"] }, description: "The permission. Administrators have every one." },
  },
  outputs: {
    result: { type: "boolean", label: "Has it", description: "Whether they have it." },
  },
  run({ fields, ctx }) {
    const member = fields.member;
    if (!member?.roles) throw new Error("Connect a member");
    if (fields.check === "permission") {
      const p = member.permissions;
      return { result: Boolean(p?.has("Administrator") || p?.has(fields.permission)) };
    }
    const roleId = ctx.fluxer.id(fields.role, "role");
    if (!roleId) throw new Error("Connect the role or type its id");
    return { result: member.roles.has(roleId) };
  },
};
