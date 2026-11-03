module.exports = {
  id: "core.wait",
  kind: "action",
  info: {
    name: "Wait",
    description: "Pauses this branch for a number of seconds, then continues.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "time",
    aliases: ["Delay", "Sleep", "Pause", "Warten"],
  },
  look: {
    icon: "clock",
  },
  fields: {
    seconds: { type: "number", default: 1, min: 0, step: 0.5, description: "How long to wait." },
  },
  async run({ fields, ctx }) {
    await ctx.util.sleep(Math.max(0, fields.seconds) * 1000);
  },
};
