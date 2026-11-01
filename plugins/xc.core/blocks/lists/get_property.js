module.exports = {
  id: "core.get_property",
  name: "Get Property",
  description: "Reads a property from an object, such as `username` from a user. Use dots for deeper ones: `guild.name`.",
  author: "XC Team",
  version: "1.0.0",
  category: "lists",
  icon: "tag",
  kind: "value",
  tags: ["object", "field", "key", "json"],
  fields: {
    object: { type: "object", pin: "only", required: true, description: "The object to read from." },
    path: { type: "text", pin: false, required: true, default: "", label: "Property", description: "The property name, such as id or guild.name." },
  },
  outputs: {
    value: { type: "any", description: "The property's value." },
  },
  define({ connected }) {
    // Known object shapes offer their keys.
    const shape = circuit.types.shape(connected.object);
    return shape ? { set: { path: { type: "select", choices: circuit.object.keysToChoices(shape) } } } : {};
  },
  run({ fields }) {
    let v = fields.object;
    for (const k of String(fields.path || "").split(".").filter(Boolean)) v = v == null ? undefined : v[k];
    return { value: v };
  },
};
