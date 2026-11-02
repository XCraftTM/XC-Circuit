const OPS = { and: "A and B", or: "A or B", not: "Not A", xor: "Only one of A, B" };

module.exports = {
  id: "core.logic",
  name: "And / Or / Not",
  description: "Combines yes/no values.",
  author: "XC Team",
  version: "1.0.0",
  category: "logic",
  icon: "scale",
  kind: "value",
  tags: ["boolean", "and", "or", "not"],
  fields: {
    a: { type: "boolean", pin: true, description: "The first value." },
    op: { type: "select", pin: false, default: "and", choices: OPS, label: "Operation", description: "How the two values are combined: both true, either true, and so on." },
    b: { type: "boolean", pin: true, description: "The second value.", show: { op: ["and", "or", "xor"] } },
  },
  outputs: {
    result: { type: "boolean", description: "The result." },
  },
  run({ fields: { a, op, b } }) {
    const result = { and: a && b, or: a || b, not: !a, xor: Boolean(a) !== Boolean(b) }[op];
    return { result: Boolean(result) };
  },
};
