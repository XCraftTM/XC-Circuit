// Each tool: label, whether it needs the second input, output type ("item" = element type).
const TOOLS = {
  length: ["Length", false, "number"],
  first: ["First item", false, "item"],
  last: ["Last item", false, "item"],
  at: ["Item at position", true, "item"],
  contains: ["Contains", true, "boolean"],
  indexOf: ["Position of", true, "number"],
  add: ["Add item", true, "list"],
  remove: ["Remove item", true, "list"],
  join: ["Join into text", true, "text"],
  reverse: ["Reverse", false, "list"],
  sort: ["Sort", false, "list"],
  unique: ["Remove duplicates", false, "list"],
  random: ["Random item", false, "item"],
  shuffle: ["Shuffle", false, "list"],
  slice: ["First N items", true, "list"],
};

module.exports = {
  id: "core.list_tools",
  kind: "value",
  info: {
    name: "List Tools",
    description: "Reads or changes a list: length, first/last, contains, add, remove, join, sort, random …",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "lists",
    aliases: ["Array tools", "Shuffle", "Sort"],
    tags: ["array", "length", "join", "sort", "random", "contains"],
  },
  look: {
    icon: "repeat",
  },
  pure: false,
  fields: {
    list: { type: "list<any>", pin: "only", required: true, description: "The list." },
    tool: { type: "select", pin: false, default: "length", choices: Object.fromEntries(Object.entries(TOOLS).map(([k, [l]]) => [k, l])), description: "What to do." },
    other: { type: "any", default: "", label: "With", description: "The item, position, separator or count, depending on the tool.", show: { tool: Object.keys(TOOLS).filter((k) => TOOLS[k][1]) } },
  },
  outputs: {
    result: { type: "any", description: "The result." },
  },
  define({ values, connected }) {
    const [label, , out] = TOOLS[values.tool] || TOOLS.length;
    const element = circuit.types.elementOf(connected.list);
    const type = out === "item" ? element : out === "list" ? connected.list || "list<any>" : out;
    return { outputs: { result: { type, label, description: `${label} of the list.` } } };
  },
  run({ fields: { list, tool, other }, ctx }) {
    const l = Array.isArray(list) ? list : list == null ? [] : [...list];
    const n = Number(other);
    const result = {
      length: () => l.length,
      first: () => l[0],
      last: () => l[l.length - 1],
      at: () => l.at(n),
      contains: () => l.some((x) => x == other),
      indexOf: () => l.findIndex((x) => x == other),
      add: () => [...l, other],
      remove: () => l.filter((x) => x != other),
      join: () => l.map((x) => String(x ?? "")).join(other ?? ", "),
      reverse: () => [...l].reverse(),
      sort: () => [...l].sort((a, b) => (typeof a === "number" && typeof b === "number" ? a - b : String(a).localeCompare(String(b)))),
      unique: () => [...new Set(l)],
      random: () => l[Math.floor(Math.random() * l.length)],
      shuffle: () => ctx.util.shuffle(l),
      slice: () => l.slice(0, n),
    }[tool]();
    return { result };
  },
};
