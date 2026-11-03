const SCOPES = { run: "This run only", workspace: "This workspace", global: "Whole bot" };

module.exports = {
  id: "core.set_variable",
  kind: "action",
  info: {
    name: "Set Variable",
    description: "Remembers a value under a name until the bot stops. Use Stored Data to keep it after restarts.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "variables",
    aliases: ["Save variable", "Variable setzen"],
    tags: ["var", "remember", "temp"],
  },
  look: {
    icon: "function",
  },
  fields: {
    name: { type: "text", pin: false, required: true, default: "", description: "The variable name." },
    scope: { type: "select", pin: false, default: "run", choices: SCOPES, description: "Who can read it." },
    value: { type: "any", pin: true, description: "The value to remember." },
  },
  run({ fields, ctx }) {
    ctx.vars[fields.scope].set(fields.name, fields.value);
  },
};
