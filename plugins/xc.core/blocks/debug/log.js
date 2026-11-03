const LEVELS = { info: "Info", success: "Success", warn: "Warning", error: "Error", debug: "Debug" };

module.exports = {
  id: "core.log",
  kind: "action",
  info: {
    name: "Log",
    description: "Writes a line to the Console panel. ${…} holds JavaScript, such as ${user.username}.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "debug",
  },
  look: {
    icon: "terminal",
  },
  fields: {
    message: { type: "text", template: true, default: "", description: "What to write." },
    level: { type: "select", default: "info", pin: false, choices: LEVELS, description: "How important it is." },
  },
  run({ fields, ctx }) {
    ctx.log[fields.level](String(fields.message ?? ""));
  },
};
