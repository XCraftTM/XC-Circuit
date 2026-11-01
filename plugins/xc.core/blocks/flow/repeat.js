module.exports = {
  id: "core.repeat",
  name: "Repeat",
  description: "Runs Each a number of times, then Done.",
  author: "XC Team",
  version: "1.0.0",
  category: "flow",
  icon: "repeat",
  kind: "action",
  flows: ["each", "done"],
  tags: ["loop", "times", "for"],
  fields: {
    times: { type: "number", default: 3, min: 0, step: 1, description: "How many times." },
  },
  outputs: {
    count: { type: "number", description: "Which round this is, starting at 1." },
  },
  async run({ fields, ctx }) {
    const n = Math.min(10000, Math.max(0, Math.floor(fields.times || 0)));
    for (let i = 1; i <= n; i++) await ctx.next("each", { count: i });
    await ctx.next("done");
  },
};
