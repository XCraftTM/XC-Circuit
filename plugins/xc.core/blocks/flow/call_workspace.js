module.exports = {
  id: "core.call",
  name: "Call",
  description: "Runs the When Called block with the same name (in any workspace), waits for it, and gives back what it returns. Like a function.",
  author: "XC Team",
  version: "1.0.0",
  category: "flow",
  icon: "function",
  kind: "action",
  tags: ["function", "subroutine", "reuse", "workspace"],
  fields: {
    name: { type: "text", required: true, default: "my-function", description: "The When Called block's name." },
    data: { type: "any", pin: true, description: "What to hand over (a value, a list or an object)." },
  },
  outputs: {
    result: { type: "any", description: "What the called flow gave back with Return." },
  },
  async run({ fields, ctx }) {
    return { result: await ctx.runtime.call(String(fields.name), fields.data) };
  },
};
