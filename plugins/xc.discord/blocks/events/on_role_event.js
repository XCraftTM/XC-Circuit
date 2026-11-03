const KINDS = { created: "Created", deleted: "Deleted", updated: "Changed" };

module.exports = {
  id: "discord.on_role_event",
  kind: "event",
  info: {
    name: "Role Created / Deleted",
    description: "Runs when a role is created, deleted or changed. For server logs.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events",
    tags: ["log", "role update", "server log"],
  },
  look: {
    icon: "tag",
  },
  requires: {
    intents: ["Guilds"],
  },
  fields: {
    kind: { type: "select", pin: false, default: "created", label: "When", choices: KINDS, description: "Which event runs it." },
  },
  outputs: {
    role: { type: "discord.role", description: "The role." },
    beforeName: { type: "text", label: "Name before", description: "Its name before a change." },
    permissionsChanged: { type: "boolean", label: "Permissions changed", description: "For changes: whether its permissions changed." },
    server: { type: "discord.guild", description: "The server." },
  },
  listen(ctx, emit) {
    const c = ctx.discord.client;
    const kind = ctx.fields.kind;
    if (kind === "updated") {
      c.on("roleUpdate", (old, now) => emit({ role: now, beforeName: old.name, permissionsChanged: old.permissions.bitfield !== now.permissions.bitfield, server: now.guild }));
    } else {
      c.on(kind === "deleted" ? "roleDelete" : "roleCreate", (role) => emit({ role, beforeName: role.name, permissionsChanged: false, server: role.guild }));
    }
  },
};
