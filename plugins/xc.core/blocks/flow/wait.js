module.exports = {
  id: "core.wait",
  name: "Wait",
  description: "Pauses this branch for a number of seconds, then continues.",
  author: "XC Team",
  version: "1.0.0",
  category: "flow",
  icon: "clock",
  kind: "action",
  fields: {
    seconds: { type: "number", default: 1, min: 0, step: 0.5, description: "How long to wait." },
  },
  async run({ fields, ctx }) {
    await ctx.util.sleep(Math.max(0, fields.seconds) * 1000);
  },
};
