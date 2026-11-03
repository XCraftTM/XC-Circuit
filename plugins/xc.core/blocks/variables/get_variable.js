const SCOPES = { run: "This run only", workspace: "This workspace", global: "Whole bot" };

module.exports = {
  id: "core.get_variable",
  kind: "value",
  info: {
    name: "Get Variable",
    description: "Reads a value remembered with Set Variable.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "variables",
    aliases: ["Read variable", "Variable lesen"],
    tags: ["var", "read"],
  },
  look: {
    icon: "function",
  },
  pure: false,
  fields: {
    name: { type: "text", pin: false, required: true, default: "", description: "The variable name." },
    scope: { type: "select", pin: false, default: "run", choices: SCOPES, description: "Where it was set." },
    fallback: { type: "any", advanced: true, label: "If missing", description: "Returned when the variable is not set." },
  },
  outputs: {
    value: { type: "any", description: "The value, or the fallback." },
  },
  run({ fields, ctx }) {
    return { value: ctx.vars[fields.scope].get(fields.name, fields.fallback) };
  },
};
