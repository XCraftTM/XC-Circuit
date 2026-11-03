const OPS = { add: "+ Add", subtract: "- Subtract", multiply: "× Multiply", divide: "÷ Divide", modulo: "% Remainder", power: "^ Power" };

module.exports = {
  id: "core.math",
  kind: "value",
  info: {
    name: "Math",
    description: "Calculates with two numbers.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "math",
  },
  look: {
    icon: "hash",
  },
  fields: {
    a: { type: "number", default: 0, description: "The first number." },
    op: { type: "select", default: "add", pin: false, choices: OPS, label: "Operation", description: "What to do with the two numbers." },
    b: { type: "number", default: 0, description: "The second number." },
  },
  outputs: {
    result: { type: "number", description: "The result." },
  },
  run({ fields: { a, op, b } }) {
    const result = { add: a + b, subtract: a - b, multiply: a * b, divide: a / b, modulo: a % b, power: a ** b }[op];
    if (!Number.isFinite(result)) throw new Error(`${op} of ${a} and ${b} is not a finite number`);
    return { result };
  },
};
