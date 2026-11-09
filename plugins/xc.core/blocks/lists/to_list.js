const TAKE = { values: "The items", keys: "Their keys (ids)", pairs: "Keys and items" };

/** Whether `v` can be walked item by item (but isn't text). */
function iterable(v) {
  return v != null && typeof v === "object" && typeof v[Symbol.iterator] === "function";
}

module.exports = {
  id: "core.to_list",
  kind: "value",
  info: {
    name: "Collection to List",
    description: "Turns a Collection (a Discord library's members, roles, channels or messages by id), a Map, a Set or an object into a list. Blocks already give lists; this is for values from JavaScript, such as ${message.mentions.users}.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "lists",
    aliases: ["To list", "Map to list", "Set to list", "Object to list", "Array from"],
    tags: ["collection", "array", "map", "set", "values", "keys", "entries", "convert"],
  },
  look: {
    icon: "repeat",
  },
  fields: {
    value: { type: "any", pin: true, required: true, default: "", description: "The Collection, Map, Set or object (or a list, given as it is)." },
    take: { type: "select", pin: false, default: "values", choices: TAKE, label: "Take", description: "For a Collection, Map or object: its items, their keys, or both (each as { key, value })." },
  },
  outputs: {
    list: { type: "list<any>", description: "The list." },
    count: { type: "number", description: "How many items it has." },
  },
  define({ values, connected }) {
    const element = connected.value && circuit.types.elementOf ? circuit.types.elementOf(connected.value) : null;
    const type = values.take === "keys" ? "list<text>" : values.take === "pairs" ? "list<object>" : element && element !== "any" ? `list<${element}>` : "list<any>";
    return { outputs: { list: { type, description: "The list." } } };
  },
  run({ fields }) {
    const v = fields.value;
    const take = fields.take || "values";
    let entries;
    if (v == null || v === "") return { list: [], count: 0 };
    if (Array.isArray(v)) return { list: v, count: v.length };
    if (v instanceof Map) entries = [...v.entries()];
    else if (v instanceof Set || (iterable(v) && typeof v !== "string")) {
      const items = [...v];
      return { list: items, count: items.length };
    } else if (typeof v === "object") entries = Object.entries(v);
    else return { list: [v], count: 1 };
    const list = entries.map(([key, value]) => (take === "keys" ? String(key) : take === "pairs" ? { key, value } : value));
    return { list, count: list.length };
  },
};
