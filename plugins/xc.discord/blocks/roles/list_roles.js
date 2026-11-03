module.exports = {
  id: "discord.list_roles",
  kind: "value",
  info: {
    name: "List Roles",
    description: "Every role of a server, highest first.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "roles",
    aliases: ["All roles"],
  },
  look: {
    icon: "tag",
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    everyone: { type: "boolean", default: false, label: "Include @everyone", description: "Also list the @everyone role." },
  },
  outputs: {
    roles: { type: "list<discord.role>", description: "The roles." },
    count: { type: "number", description: "How many." },
  },
  run({ fields }) {
    const roles = [...fields.server.roles.cache.values()]
      .filter((r) => fields.everyone || r.id !== fields.server.id)
      .sort((a, b) => b.position - a.position);
    return { roles, count: roles.length };
  },
};
