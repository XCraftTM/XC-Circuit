module.exports = {
  id: "core.emit_signal",
  kind: "action",
  info: {
    name: "Emit Signal",
    description: "Starts every On Signal block with the same name. Replaces Emitter/Receiver.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "calls",
    aliases: ["Send signal", "Broadcast", "Signal senden"],
  },
  look: {
    icon: "send",
  },
  fields: {
    name: { type: "text", pin: false, required: true, default: "", description: "The signal name." },
    data: { type: "any", pin: true, description: "A value to send along." },
  },
  run({ fields, ctx }) {
    ctx.signal.emit(fields.name, fields.data);
  },
};
