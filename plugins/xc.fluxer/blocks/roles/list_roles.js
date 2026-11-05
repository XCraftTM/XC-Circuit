module.exports = {
  id: "fluxer.list_roles",
  kind: "action",
  info: {
    name: "Server Roles",
    description: "Gets every role of a server, highest first.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "roles",
    aliases: ["List roles", "All roles"],
  },
  look: {
    icon: "tag",
  },
  fields: {
    server: { type: "fluxer.server", pin: "only", required: true, description: "The server." },
  },
  outputs: {
    roles: { type: "list<fluxer.role>", description: "Every role, highest first." },
    names: { type: "list<text>", description: "Their names." },
    count: { type: "number", description: "How many." },
  },
  async run({ fields: f }) {
    if (!f.server) throw new Error("Connect the server");
    const roles = [...(await f.server.fetchRoles())].sort((a, b) => (b.position ?? 0) - (a.position ?? 0));
    return { roles, names: roles.map((r) => r.name), count: roles.length };
  },
};
