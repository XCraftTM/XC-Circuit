const SCOPES = { global: "Whole bot", server: "Server", user: "User", member: "Member (server + user)", channel: "Channel" };

module.exports = {
  id: "core.set_stored",
  name: "Set Stored Value",
  description: "Saves a value so it survives bot restarts.",
  author: "XC Team",
  version: "1.0.0",
  category: "data",
  icon: "database",
  kind: "action",
  fields: {
    scope: { type: "select", default: "user", pin: false, choices: SCOPES, description: "Who the value belongs to." },
    target: {
      type: "any",
      pin: true,
      description: "The user, server or channel (or its id) the value belongs to.",
      show: { scope: ["server", "user", "member", "channel"] },
    },
    key: { type: "text", default: "xp", required: true, description: "The name to save it under." },
    value: { type: "any", pin: true, description: "The value to save." },
  },
  async run({ fields, ctx }) {
    const target = fields.target?.id ?? fields.target;
    const store = fields.scope === "global" ? ctx.store.global() : ctx.store[fields.scope](target);
    await store.set(fields.key, fields.value);
  },
};
