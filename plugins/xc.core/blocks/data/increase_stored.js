const SCOPES = { global: "Whole bot", server: "Server", user: "User", member: "Member (server + user)", channel: "Channel" };

module.exports = {
  id: "core.increase_stored",
  kind: "action",
  info: {
    name: "Increase Stored Value",
    description: "Adds to a stored number, starting from 0. Use a negative amount to subtract.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "data",
    aliases: ["Add XP", "Increment", "Counter", "Erhöhen", "Zähler"],
  },
  look: {
    icon: "plus-circle",
  },
  fields: {
    scope: { type: "select", default: "user", pin: false, choices: SCOPES, description: "Who the value belongs to." },
    target: {
      type: "any",
      pin: true,
      description: "The user, server or channel (or its id).",
      show: { scope: ["server", "user", "member", "channel"] },
    },
    key: { type: "text", default: "xp", required: true, description: "The name of the stored number." },
    by: { type: "number", default: 1, description: "How much to add." },
  },
  outputs: {
    value: { type: "number", description: "The new value." },
  },
  async run({ fields, ctx }) {
    const target = fields.target?.id ?? fields.target;
    const store = fields.scope === "global" ? ctx.store.global() : ctx.store[fields.scope](target);
    return { value: await store.inc(fields.key, fields.by) };
  },
};
