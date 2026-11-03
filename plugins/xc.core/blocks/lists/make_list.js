module.exports = {
  id: "core.make_list",
  kind: "value",
  info: {
    name: "Make List",
    description: "Puts connected values into a list, in wire order. Text typed below adds items too (one per line).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "lists",
    tags: ["array", "create"],
  },
  look: {
    icon: "repeat",
  },
  fields: {
    items: { type: "any", multi: true, pin: true, description: "Values to put in the list." },
    lines: { type: "textarea", pin: false, default: "", label: "Text items", description: "One item per line." },
  },
  outputs: {
    list: { type: "list<any>", description: "The list." },
  },
  run({ fields }) {
    const typed = String(fields.lines || "")
      .split(/\r?\n/)
      .map((s) => s.trim())
      .filter(Boolean);
    return { list: [...(fields.items || []).filter((v) => v !== undefined), ...typed] };
  },
};
