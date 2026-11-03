module.exports = {
  id: "core.set_property",
  kind: "value",
  info: {
    name: "Set Property",
    description: "A copy of an object with one property set (or removed). Dots reach deeper: settings.color.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "lists",
    aliases: ["Change field"],
    tags: ["object", "assign", "update", "json"],
  },
  look: {
    icon: "function",
  },
  fields: {
    object: { type: "object", pin: true, description: "The object (it isn't changed itself)." },
    path: { type: "text", template: false, required: true, default: "name", description: "The property, such as name or settings.color." },
    value: { type: "any", pin: true, description: "The new value. Nothing connected removes the property." },
  },
  outputs: {
    result: { type: "object", description: "The changed copy." },
  },
  run({ fields: f }) {
    const root = structuredClone(f.object && typeof f.object === "object" ? f.object : {});
    const parts = String(f.path).split(".").filter(Boolean);
    let at = root;
    for (const p of parts.slice(0, -1)) at = at[p] && typeof at[p] === "object" ? at[p] : (at[p] = {});
    const last = parts[parts.length - 1];
    if (f.value === undefined) delete at[last];
    else at[last] = f.value;
    return { result: root };
  },
};
