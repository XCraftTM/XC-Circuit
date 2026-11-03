const SCOPES = { global: "Whole bot", server: "Server", user: "User", member: "Member (server + user)", channel: "Channel" };

module.exports = {
  id: "core.delete_stored",
  kind: "action",
  info: {
    name: "Delete Stored Value",
    description: "Removes a saved value, as if it had never been set.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "data",
    aliases: ["Remove data"],
    tags: ["remove", "reset", "clear"],
  },
  look: {
    icon: "database",
  },
  fields: {
    scope: { type: "select", default: "user", pin: false, choices: SCOPES, description: "Who the value belongs to." },
    target: { type: "any", pin: true, description: "The user, server or channel (or its id).", show: { scope: ["server", "user", "member", "channel"] } },
    server: { type: "any", pin: "only", description: "The member's server.", show: { scope: ["member"] } },
    key: { type: "text", default: "xp", required: true, description: "The name of the stored value." },
  },
  outputs: {
    existed: { type: "boolean", description: "Whether there was a value to delete." },
  },
  async run({ fields: f, ctx }) {
    const id = (v) => v?.id ?? v;
    const store = f.scope === "global" ? ctx.store.global() : f.scope === "member" ? ctx.store.member(id(f.server ?? f.target?.guild), id(f.target)) : ctx.store[f.scope](id(f.target));
    const existed = await store.has(f.key);
    await store.delete(f.key);
    return { existed };
  },
};
