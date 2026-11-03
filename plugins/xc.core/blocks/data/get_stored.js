const SCOPES = { global: "Whole bot", server: "Server", user: "User", member: "Member (server + user)", channel: "Channel" };

module.exports = {
  id: "core.get_stored",
  kind: "value",
  info: {
    name: "Get Stored Value",
    description: "Reads a value saved with Set Stored Value. Survives bot restarts.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "data",
    aliases: ["Load", "Database get", "Gespeicherten Wert lesen", "Laden"],
  },
  look: {
    icon: "database",
  },
  pure: false,
  fields: {
    scope: { type: "select", default: "user", pin: false, choices: SCOPES, description: "Who the value belongs to." },
    target: {
      type: "any",
      pin: true,
      description: "The user, server or channel (or its id) the value belongs to.",
      show: { scope: ["server", "user", "member", "channel"] },
    },
    key: { type: "text", default: "xp", required: true, description: "The name of the stored value." },
    fallback: { type: "any", default: 0, advanced: true, label: "If missing", description: "Returned when nothing is stored yet." },
  },
  outputs: {
    value: { type: "any", description: "The stored value, or the fallback." },
  },
  async run({ fields, ctx }) {
    const target = fields.target?.id ?? fields.target;
    const store = fields.scope === "global" ? ctx.store.global() : ctx.store[fields.scope](target);
    return { value: await store.get(fields.key, fields.fallback) };
  },
};
