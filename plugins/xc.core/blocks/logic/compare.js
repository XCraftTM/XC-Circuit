const OPS = {
  eq: "equals",
  ne: "does not equal",
  gt: "is greater than",
  lt: "is less than",
  gte: "is at least",
  lte: "is at most",
  contains: "contains",
  starts: "starts with",
};

module.exports = {
  id: "core.compare",
  kind: "value",
  info: {
    name: "Compare",
    description: "Compares two values and gives true or false.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "numbers",
    aliases: ["Equals", "Greater than", "Vergleichen"],
  },
  look: {
    icon: "scale",
  },
  fields: {
    a: { type: "any", pin: true, description: "The value to check." },
    op: { type: "select", default: "eq", pin: false, choices: OPS, label: "Check", description: "How to compare." },
    b: { type: "any", default: "", description: "The value to compare with." },
  },
  outputs: {
    result: { type: "boolean", description: "Whether the comparison is true." },
  },
  run({ fields: { a, op, b } }) {
    const checks = {
      eq: () => a == b,
      ne: () => a != b,
      gt: () => a > b,
      lt: () => a < b,
      gte: () => a >= b,
      lte: () => a <= b,
      contains: () => String(a).includes(String(b)),
      starts: () => String(a).startsWith(String(b)),
    };
    return { result: Boolean(checks[op]()) };
  },
};
