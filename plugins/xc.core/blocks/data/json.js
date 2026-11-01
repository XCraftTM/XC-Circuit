module.exports = {
  id: "core.json",
  name: "JSON",
  description: "Turns text into an object (parse) or an object into text (stringify).",
  author: "XC Team",
  version: "1.0.0",
  category: "data",
  icon: "database",
  kind: "value",
  tags: ["parse", "stringify", "object", "text"],
  fields: {
    input: { type: "any", pin: true, description: "Text to parse, or a value to turn into text." },
    direction: { type: "select", pin: false, default: "parse", choices: { parse: "Text → object", stringify: "Object → text" } },
    pretty: { type: "boolean", pin: false, default: true, show: { direction: ["stringify"] } },
  },
  outputs: {
    result: { type: "any", description: "The result." },
  },
  define({ values }) {
    return { outputs: { result: { type: values.direction === "stringify" ? "text" : "any", description: "The result." } } };
  },
  run({ fields }) {
    if (fields.direction === "stringify") return { result: JSON.stringify(fields.input, null, fields.pretty ? 2 : 0) };
    return { result: typeof fields.input === "string" ? JSON.parse(fields.input) : fields.input };
  },
};
