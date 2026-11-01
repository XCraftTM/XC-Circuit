module.exports = {
  id: "core.on_signal",
  name: "On Signal",
  description: "Runs when Emit Signal sends a signal with the same name, from any workspace.",
  author: "XC Team",
  version: "1.0.0",
  category: "events",
  icon: "bolt",
  kind: "event",
  fields: {
    name: { type: "text", pin: false, required: true, default: "", description: "The signal name to listen for." },
  },
  outputs: {
    data: { type: "any", description: "The value sent with the signal." },
  },
  listen(ctx, emit) {
    ctx.runtime.onSignal(ctx.fields.name, (data) => emit({ data }));
  },
};
