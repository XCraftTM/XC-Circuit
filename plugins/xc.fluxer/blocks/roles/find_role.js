module.exports = {
  id: "fluxer.find_role",
  kind: "action",
  info: {
    name: "Find Role",
    description: "Finds a role in a server by its name, id or mention, and tells its name, color and position.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "roles",
    aliases: ["Get role", "Role by name", "Role info"],
  },
  look: {
    icon: "key",
  },
  fields: {
    server: { type: "fluxer.server", pin: "only", required: true, description: "The server." },
    role: { type: "text", default: "", description: "Its name (any capitals), id, or <@&id>." },
  },
  outputs: {
    role: { type: "fluxer.role?", description: "The role. Empty when not found." },
    found: { type: "boolean", description: "Whether there is such a role." },
    name: { type: "text", description: "Its name." },
    id: { type: "text", description: "Its id." },
    color: { type: "text", description: "Its color as #RRGGBB." },
    position: { type: "number", description: "Its place in the role list: higher roles have bigger numbers." },
    mention: { type: "text", description: "<@&id>, which pings the role in a message." },
  },
  async run({ fields, ctx }) {
    const server = fields.server;
    if (!server) throw new Error("Connect the server");
    const wanted = String(fields.role ?? "").trim();
    if (!wanted) throw new Error("Type the role's name or id");
    const byId = ctx.fluxer.id(wanted, "role");
    const match = (list) => [...list].find((r) => r.id === byId || r.name?.toLowerCase() === wanted.toLowerCase());
    let role = match(server.roles?.values?.() ?? []);
    if (!role) role = match(await server.fetchRoles());
    if (!role) return { role: null, found: false, name: "", id: "", color: "", position: 0, mention: "" };
    return {
      role,
      found: true,
      name: role.name,
      id: role.id,
      color: `#${Number(role.color || 0).toString(16).padStart(6, "0").toUpperCase()}`,
      position: role.position ?? 0,
      mention: `<@&${role.id}>`,
    };
  },
};
