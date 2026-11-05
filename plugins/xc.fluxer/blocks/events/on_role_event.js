const WHEN = { create: "Created", update: "Changed", delete: "Deleted" };

module.exports = {
  id: "fluxer.on_role_event",
  kind: "event",
  info: {
    name: "Role Created, Changed or Deleted",
    description: "Runs when a role is made, changed (name, color, permissions …) or deleted in a server.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "roles",
    aliases: ["On role", "Role created", "Role deleted", "Role updated"],
  },
  look: {
    icon: "tag",
  },
  fields: {
    when: { type: "select", pin: false, default: "create", choices: WHEN, label: "When", description: "Which change." },
  },
  outputs: {
    role: { type: "fluxer.role?", description: "The role (as it is now). Empty when a deleted role wasn't known." },
    before: { type: "fluxer.role?", description: "For changes: the role as it was, when known." },
    roleId: { type: "text", label: "Role id", description: "The role's id." },
    server: { type: "fluxer.server?", description: "The server." },
  },
  listen(ctx, emit) {
    const client = ctx.fluxer.client;
    const server = (guildId) => client.guilds?.get?.(guildId) ?? null;
    if (ctx.fields.when === "update") {
      client.on("guildRoleUpdate", (before, role) => emit({ role, before: before ?? null, roleId: role.id, server: server(role.guildId) }));
    } else if (ctx.fields.when === "delete") {
      client.on("guildRoleDelete", (role, guildId, roleId) => emit({ role: role ?? null, before: null, roleId: String(roleId ?? role?.id ?? ""), server: server(guildId) }));
    } else {
      client.on("guildRoleCreate", (role) => emit({ role, before: null, roleId: role.id, server: server(role.guildId) }));
    }
  },
};
