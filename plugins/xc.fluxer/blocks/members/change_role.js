const ACTIONS = { add: "Give the role", remove: "Take the role away", toggle: "Give or take away (switch)" };

module.exports = {
  id: "fluxer.change_role",
  kind: "action",
  info: {
    name: "Give or Take Role",
    description: "Gives a member a role, takes it away, or switches it.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "roles",
    aliases: ["Add role", "Remove role", "Toggle role", "Auto role"],
  },
  look: {
    icon: "key",
  },
  requires: {
    permissions: ["ManageRoles"],
  },
  fields: {
    member: { type: "fluxer.member", pin: "only", required: true, description: "Who." },
    action: { type: "select", pin: false, default: "add", choices: ACTIONS, description: "What to do." },
    role: { type: "any", default: "", description: "The role: connect one, or type its id or <@&id>." },
    reason: { type: "text", default: "", advanced: true, description: "Why, for the server's audit log." },
  },
  outputs: {
    has: { type: "boolean", label: "Has it now", description: "Whether the member has the role afterwards." },
  },
  async run({ fields, ctx }) {
    const member = fields.member;
    if (!member?.roles) throw new Error("Connect a member");
    const roleId = ctx.fluxer.id(fields.role, "role");
    if (!roleId) throw new Error("Connect the role or type its id");
    const has = member.roles.has(roleId);
    const give = fields.action === "toggle" ? !has : fields.action !== "remove";
    if (give && !has) await member.roles.add(roleId);
    if (!give && has) await member.roles.remove(roleId);
    return { has: give };
  },
};
