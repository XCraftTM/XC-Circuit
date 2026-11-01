module.exports = {
  id: "core.if",
  name: "If",
  description: "Continues on True or False depending on a condition.",
  author: "XC Team",
  version: "1.0.0",
  category: "flow",
  icon: "branch",
  kind: "action",
  flows: ["true", "false"],
  fields: {
    condition: { type: "boolean", pin: true, description: "The yes/no value to check." },
  },
  async run({ fields, ctx }) {
    await ctx.next(fields.condition ? "true" : "false");
  },
};
