module.exports = {
  id: "core.switch",
  kind: "action",
  info: {
    name: "Switch",
    description: "Continues on the output whose case matches the value. Type one case per line; Other runs when none match.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "flow",
    tags: ["case", "match", "choose"],
  },
  look: {
    icon: "branch",
  },
  flows: ["other"],
  fields: {
    value: { type: "any", pin: true, description: "The value to check." },
    cases: { type: "textarea", pin: false, default: "a\nb", description: "One case per line. Each becomes an Action output." },
    ignoreCase: { type: "boolean", default: true, advanced: true, label: "Ignore upper/lower case", description: "Treat \"Yes\" and \"yes\" as the same when comparing." },
  },
  define({ values }) {
    const cases = String(values.cases || "")
      .split(/\r?\n/)
      .map((s) => s.trim())
      .filter(Boolean);
    return { flows: [...new Set(cases).values(), "other"] };
  },
  async run({ fields, ctx }) {
    const norm = (s) => (fields.ignoreCase ? String(s).toLowerCase() : String(s));
    const cases = String(fields.cases || "")
      .split(/\r?\n/)
      .map((s) => s.trim())
      .filter(Boolean);
    const hit = cases.find((c) => norm(c) === norm(fields.value ?? ""));
    await ctx.next(hit ?? "other");
  },
};
