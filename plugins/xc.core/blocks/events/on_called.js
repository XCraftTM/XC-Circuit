module.exports = {
  id: "core.on_called",
  kind: "event",
  info: {
    name: "When Called",
    description: "Runs when a Call block with the same name runs. End the flow with Return to give a value back.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "calls",
    aliases: ["Function", "Subroutine"],
    tags: ["function", "subroutine", "reuse"],
  },
  look: {
    icon: "function",
  },
  fields: {
    name: { type: "text", pin: false, required: true, default: "my-function", description: "The name Call blocks use." },
  },
  outputs: {
    data: { type: "any", description: "What the Call block handed over." },
  },
  listen(ctx, emit) {
    ctx.runtime.onCall(String(ctx.fields.name), (data) => emit({ data }));
  },
};
