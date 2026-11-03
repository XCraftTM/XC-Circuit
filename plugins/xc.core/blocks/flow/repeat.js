module.exports = {
  id: "core.repeat",
  kind: "action",
  info: {
    name: "Repeat",
    description: "Runs Each a number of times, then Done.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "flow",
    aliases: ["Loop N times"],
    tags: ["loop", "times", "for"],
  },
  look: {
    icon: "repeat",
  },
  flows: ["each", "done"],
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
