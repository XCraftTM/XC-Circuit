const LEVELS = { info: "Info", success: "Success", warn: "Warning", error: "Error", debug: "Debug" };

module.exports = {
  id: "core.log",
  name: "Log",
  description: "Writes a line to the Console panel. Use {name} placeholders for connected values.",
  author: "XC Team",
  version: "1.0.0",
  category: "debug",
  icon: "terminal",
  kind: "action",
  fields: {
    message: { type: "text", template: true, default: "", description: "What to write." },
    level: { type: "select", default: "info", pin: false, choices: LEVELS, description: "How important it is." },
  },
  run({ fields, ctx }) {
    ctx.log[fields.level](String(fields.message ?? ""));
  },
};
