module.exports = {
  id: "core.return",
  kind: "action",
  info: {
    name: "Return",
    description: "Gives a value back to the Call block that started this flow.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "flow",
    tags: ["function", "result", "give back"],
  },
  look: {
    icon: "function",
  },
  fields: {
    value: { type: "any", pin: true, description: "What to give back." },
  },
  run({ fields, ctx }) {
    ctx.returnValue(fields.value);
  },
};
