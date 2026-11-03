module.exports = {
  id: "discord.find_role",
  kind: "action",
  info: {
    name: "Find Role",
    description: "Looks up a role of a server by id, name or mention.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "roles",
    tags: ["get", "lookup", "fetch"],
  },
  look: {
    icon: "tag",
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server the role is in." },
    role: { type: "text", required: true, default: "", label: "Id or name", description: "A role id, a name such as Moderator, or a mention." },
  },
  outputs: {
    role: { type: "discord.role", description: "The role. Fails when it isn't found." },
  },
  async run({ fields }) {
    const key = String(fields.role).trim().replace(/^<@&(\d+)>$/, "$1").replace(/^@/, "");
    const roles = fields.server.roles;
    const found = /^\d{15,}$/.test(key) ? await roles.fetch(key).catch(() => null) : roles.cache.find((r) => r.name.toLowerCase() === key.toLowerCase());
    if (!found) throw new Error(`No role "${key}" in ${fields.server.name}`);
    return { role: found };
  },
};
