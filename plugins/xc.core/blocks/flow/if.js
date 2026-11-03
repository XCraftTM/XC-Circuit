module.exports = {
  id: "core.if",
  kind: "action",
  info: {
    name: "If",
    description: "Continues on True or False depending on a condition.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "flow",
    aliases: ["Condition", "Branch", "Wenn", "Bedingung"],
  },
  look: {
    icon: "branch",
  },
  flows: ["true", "false"],
  fields: {
    condition: { type: "boolean", pin: true, description: "The yes/no value to check." },
  },
  async run({ fields, ctx }) {
    await ctx.next(fields.condition ? "true" : "false");
  },
};
