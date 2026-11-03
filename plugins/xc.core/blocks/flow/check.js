// The same checks as Compare (both use ctx.util.compare).
const CHECKS = {
  eq: "equals",
  ne: "does not equal",
  gt: "is greater than",
  lt: "is less than",
  gte: "is at least",
  lte: "is at most",
  between: "is between",
  contains: "contains",
  not_contains: "does not contain",
  starts: "starts with",
  ends: "ends with",
  matches: "matches the pattern",
  one_of: "is one of",
  empty: "is empty",
  not_empty: "is not empty",
  number: "is a number",
  true: "is true",
  false: "is false",
};
const WITH_VALUE = ["eq", "ne", "gt", "lt", "gte", "lte", "between", "contains", "not_contains", "starts", "ends", "matches", "one_of"];

module.exports = {
  id: "core.check",
  kind: "action",
  info: {
    name: "Check",
    description: "Compares a value and continues on True or False: equals, greater than, between, contains, matches a pattern, is empty and more.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "flow",
    aliases: ["If", "Compare", "Condition", "Check Comparison", "Equals", "Greater than", "Contains", "Is empty"],
  },
  look: {
    icon: "branch",
  },
  flows: ["true", "false"],
  fields: {
    a: { type: "any", pin: true, label: "Value", description: "The value to check: a number, text, list, date, user, role…" },
    op: { type: "select", default: "eq", pin: false, choices: CHECKS, label: "Check", description: "How to check it. Numbers (also typed as text) compare as numbers, dates by time, users, roles and channels by id." },
    b: {
      type: "any",
      default: "",
      label: "Compare with",
      show: { op: WITH_VALUE },
      description: "The value to compare with. For “is one of”: a list, or texts separated by commas. For “matches the pattern”: a regular expression, such as ^\\d+$ or /hello/i.",
    },
    c: { type: "any", default: "", label: "And", show: { op: ["between"] }, description: "The upper end: between includes both ends." },
    ignoreCase: { type: "boolean", pin: false, default: false, label: "Ignore case", show: { op: WITH_VALUE }, description: "Treat capital and small letters as the same when comparing texts." },
  },
  async run({ fields: { a, op, b, c, ignoreCase }, ctx }) {
    await ctx.next(ctx.util.compare(a, op, b, { c, ignoreCase }) ? "true" : "false");
  },
};
