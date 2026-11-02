module.exports = {
  id: "core.return",
  name: "Return",
  description: "Gives a value back to the Call block that started this flow.",
  author: "XC Team",
  version: "1.0.0",
  category: "flow",
  icon: "function",
  kind: "action",
  tags: ["function", "result", "give back"],
  fields: {
    value: { type: "any", pin: true, description: "What to give back." },
  },
  run({ fields, ctx }) {
    ctx.returnValue(fields.value);
  },
};
