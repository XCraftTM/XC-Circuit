module.exports = {
  id: "core.random_number",
  name: "Random Number",
  description: "A random number between Min and Max (both included).",
  author: "XC Team",
  version: "1.0.0",
  category: "math",
  icon: "hash",
  kind: "value",
  pure: false,
  tags: ["dice", "roll", "chance"],
  fields: {
    min: { type: "number", default: 1, description: "The smallest possible number." },
    max: { type: "number", default: 100, description: "The largest possible number." },
    whole: { type: "boolean", pin: false, default: true, label: "Whole numbers", description: "Off gives decimals." },
  },
  outputs: {
    number: { type: "number", description: "The random number." },
  },
  run({ fields: { min, max, whole } }) {
    const [lo, hi] = min <= max ? [min, max] : [max, min];
    const number = whole ? Math.floor(Math.random() * (Math.floor(hi) - Math.ceil(lo) + 1)) + Math.ceil(lo) : Math.random() * (hi - lo) + lo;
    return { number };
  },
};
