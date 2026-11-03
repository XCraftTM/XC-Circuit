const SCOPES = { global: "Whole bot", server: "Server", user: "User", member: "Member (server + user)", channel: "Channel" };

module.exports = {
  id: "core.list_stored_keys",
  kind: "value",
  info: {
    name: "List Stored Keys",
    description: "The names of every value saved for a user, server, channel or the whole bot.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "data",
    aliases: ["Keys"],
  },
  look: {
    icon: "database",
  },
  pure: false,
  fields: {
    scope: { type: "select", default: "user", pin: false, choices: SCOPES, description: "Whose values." },
    target: { type: "any", pin: true, description: "The user, server or channel (or its id).", show: { scope: ["server", "user", "member", "channel"] } },
    server: { type: "any", pin: "only", description: "The member's server.", show: { scope: ["member"] } },
    prefix: { type: "text", default: "", description: "Only names starting with this." },
  },
  outputs: {
    keys: { type: "list<text>", description: "The names." },
  },
  async run({ fields: f, ctx }) {
    const id = (v) => v?.id ?? v;
    const store = f.scope === "global" ? ctx.store.global() : f.scope === "member" ? ctx.store.member(id(f.server ?? f.target?.guild), id(f.target)) : ctx.store[f.scope](id(f.target));
    return { keys: (await store.keys()).filter((k) => k.startsWith(f.prefix || "")) };
  },
};
