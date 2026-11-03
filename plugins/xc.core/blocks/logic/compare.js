// The same checks as Check (both use ctx.util.compare).
const OPS = {
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
  id: "core.compare",
  kind: "value",
  info: {
    name: "Compare",
    description: "Compares two values and gives true or false. To continue on True or False right away, use Check.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "numbers",
    aliases: ["Equals", "Greater than", "Between", "Contains", "Is empty"],
  },
  look: {
    icon: "scale",
  },
  fields: {
    a: { type: "any", pin: true, description: "The value to check." },
    op: { type: "select", default: "eq", pin: false, choices: OPS, label: "Check", description: "How to compare. Numbers (also typed as text) compare as numbers, dates by time, users, roles and channels by id." },
    b: {
      type: "any",
      default: "",
      show: { op: WITH_VALUE },
      description: "The value to compare with. For “is one of”: a list, or texts separated by commas. For “matches the pattern”: a regular expression, such as ^\\d+$ or /hello/i.",
    },
    c: { type: "any", default: "", label: "And", show: { op: ["between"] }, description: "The upper end: between includes both ends." },
    ignoreCase: { type: "boolean", pin: false, default: false, label: "Ignore case", show: { op: WITH_VALUE }, description: "Treat capital and small letters as the same when comparing texts." },
  },
  outputs: {
    result: { type: "boolean", description: "Whether the comparison is true." },
  },
  run({ fields: { a, op, b, c, ignoreCase }, ctx }) {
    return { result: ctx.util.compare(a, op, b, { c, ignoreCase }) };
  },
};
