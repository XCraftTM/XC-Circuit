module.exports = {
  id: "core.loop_while",
  kind: "action",
  info: {
    name: "Loop While",
    description: "Runs Each again and again while a condition stays true (checked before every round), then Done.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "flow",
    tags: ["while", "until", "loop"],
  },
  look: {
    icon: "repeat",
  },
  flows: ["each", "done"],
  fields: {
    condition: { type: "boolean", pin: true, description: "Keep going while this is true. It is read again before each round." },
    max: { type: "number", default: 1000, min: 1, max: 100000, step: 1, advanced: true, label: "At most", description: "A safety limit on rounds." },
  },
  outputs: {
    count: { type: "number", description: "Which round this is, starting at 1." },
  },
  async run({ fields, ctx }) {
    const max = Math.max(1, Math.floor(fields.max || 1000));
    let f = fields;
    let i = 0;
    while (f.condition && i < max) {
      await ctx.next("each", { count: ++i });
      f = await ctx.resolve();
    }
    await ctx.next("done", { count: i });
  },
};
