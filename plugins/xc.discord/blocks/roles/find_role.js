module.exports = {
  id: "discord.find_role",
  kind: "action",
  info: {
    name: "Find Role",
    description: "Looks up a role of a server by id, name or mention.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "roles",
    aliases: ["Get role"],
    tags: ["get", "lookup", "fetch"],
  },
  look: {
    icon: "tag",
  },
  flows: ["next", "missing"],
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server the role is in." },
    role: { type: "text", required: true, default: "", label: "Id or name", description: "A role id, a name such as Moderator, or a mention." },
  },
  outputs: {
    role: { type: "discord.role", description: "The role. When it isn't found, Missing runs instead (or, with nothing connected to Missing, the block fails)." },
  },
  async run({ fields, ctx }) {
    const key = String(fields.role).trim().replace(/^<@&(\d+)>$/, "$1").replace(/^@/, "");
    const roles = fields.server.roles;
    const found = /^\d{15,}$/.test(key) ? await roles.fetch(key).catch(() => null) : roles.cache.find((r) => r.name.toLowerCase() === key.toLowerCase());
    if (!found) {
      // Missing handles it when connected; otherwise it's an error, as it always was.
      if (ctx.wired?.("missing")) return ctx.next("missing", { role: null });
      throw new Error(`No role "${key}" in ${fields.server.name}`);
    }
    await ctx.next("next", { role: found });
  },
};
