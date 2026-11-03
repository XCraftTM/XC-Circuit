const keysOf = (text) => String(text || "").split(/\r?\n|,/).map((s) => s.trim()).filter((s) => /^[A-Za-z_$][\w$]*$/.test(s));

module.exports = {
  id: "core.make_object",
  kind: "value",
  info: {
    name: "Make Object",
    description: "Builds an object from named values, such as for a web request or JSON.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "lists",
    tags: ["json", "dictionary", "map", "record", "body"],
  },
  look: {
    icon: "function",
  },
  fields: {
    keys: { type: "textarea", pin: false, template: false, default: "name\nvalue", label: "Names", description: "One name per line. Each gets an input below to type or connect its value." },
  },
  outputs: {
    object: { type: "object", description: "The object." },
  },
  define({ values }) {
    const add = {};
    for (const k of keysOf(values.keys)) add[k] = { type: "any", default: "", label: k, description: `The value of ${k}.` };
    return { add };
  },
  run({ fields }) {
    const object = {};
    for (const k of keysOf(fields.keys)) object[k] = fields[k];
    return { object };
  },
};
