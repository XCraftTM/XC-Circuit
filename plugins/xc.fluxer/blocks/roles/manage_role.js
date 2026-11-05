const ACTIONS = { create: "Create a role", edit: "Change a role", delete: "Delete a role" };
const SWITCH = { keep: "Keep as it is", on: "On", off: "Off" };

module.exports = {
  id: "fluxer.manage_role",
  kind: "action",
  info: {
    name: "Create, Change or Delete Role",
    description: "Makes a new role, changes a role's name, color, permissions and display, or deletes it. The bot's role must be above the role it changes.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "roles",
    aliases: ["Create role", "Edit role", "Delete role", "Rename role", "Role color"],
  },
  look: {
    icon: "tag",
  },
  requires: {
    permissions: ["ManageRoles"],
  },
  fields: {
    action: { type: "select", pin: false, default: "create", choices: ACTIONS, description: "What to do." },
    server: { type: "fluxer.server", pin: "only", required: true, show: { action: ["create"] }, description: "The server to make it in." },
    role: { type: "fluxer.role", pin: "only", required: true, show: { action: ["edit", "delete"] }, description: "The role." },
    name: { type: "text", default: "", show: { action: ["create", "edit"] }, description: "Its name. For changes: empty keeps it." },
    color: { type: "text", default: "", show: { action: ["create", "edit"] }, description: "#RRGGBB, such as #5865F2. Empty keeps it (or none for a new role)." },
    hoist: { type: "select", pin: false, default: "keep", choices: SWITCH, label: "Show separately", show: { action: ["create", "edit"] }, description: "List its members apart in the member list." },
    mentionable: { type: "select", pin: false, default: "keep", choices: SWITCH, show: { action: ["create", "edit"] }, description: "Whether everyone may @mention it." },
    permissions: {
      type: "text",
      default: "",
      advanced: true,
      show: { action: ["create", "edit"] },
      description: "Permission names, with commas: SendMessages, ManageMessages. Empty keeps them.",
    },
    reason: { type: "text", default: "", description: "Why, for the audit log." },
  },
  outputs: {
    role: { type: "fluxer.role?", description: "The new or changed role. Empty after deleting." },
  },
  async run({ fields: f }) {
    const reason = String(f.reason ?? "").trim() || undefined;
    if (f.action === "delete") {
      if (!f.role) throw new Error("Connect the role");
      await f.role.delete(reason);
      return { role: null };
    }
    const options = {};
    const name = String(f.name ?? "").trim();
    if (name) options.name = name.slice(0, 100);
    const color = String(f.color ?? "").trim();
    if (color) {
      const n = parseInt(color.replace(/^#/, ""), 16);
      if (!Number.isFinite(n)) throw new Error(`${color} isn't a color: use #RRGGBB`);
      options.color = n;
    }
    if (f.hoist !== "keep") options.hoist = f.hoist === "on";
    if (f.mentionable !== "keep") options.mentionable = f.mentionable === "on";
    const perms = String(f.permissions ?? "")
      .split(/[,\s]+/)
      .map((p) => p.trim())
      .filter(Boolean);
    if (perms.length) options.permissions = perms;
    if (reason) options.reason = reason;
    if (f.action === "create") {
      if (!f.server) throw new Error("Connect the server");
      if (!options.name) throw new Error("Give the role a name");
      const { hoist, mentionable, ...create } = options;
      let role = await f.server.createRole(create);
      if (hoist !== undefined || mentionable !== undefined) role = await role.edit({ hoist, mentionable });
      return { role };
    }
    if (!f.role) throw new Error("Connect the role");
    return { role: Object.keys(options).length ? await f.role.edit(options) : f.role };
  },
};
