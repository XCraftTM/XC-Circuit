// Each tool: label, whether it uses "With" and/or "Replace with", output type.
const TOOLS = {
  upper: ["UPPERCASE", false, false, "text"],
  lower: ["lowercase", false, false, "text"],
  trim: ["Trim spaces", false, false, "text"],
  length: ["Length", false, false, "number"],
  contains: ["Contains", true, false, "boolean"],
  starts: ["Starts with", true, false, "boolean"],
  ends: ["Ends with", true, false, "boolean"],
  replace: ["Replace all", true, true, "text"],
  split: ["Split into list", true, false, "list<text>"],
  words: ["Words", false, false, "list<text>"],
  slice: ["First N characters", true, false, "text"],
  number: ["To number", false, false, "number"],
  regex: ["Matches pattern", true, false, "boolean"],
};

module.exports = {
  id: "core.text_tools",
  kind: "value",
  info: {
    name: "Text Tools",
    description: "Changes or checks text: upper/lower case, trim, contains, replace, split, length …",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "text",
    aliases: ["Uppercase", "Replace", "Split", "Trim", "Textwerkzeuge"],
    tags: ["string", "replace", "split", "uppercase", "regex"],
  },
  look: {
    icon: "text",
  },
  fields: {
    text: { type: "text", pin: true, description: "The text." },
    tool: { type: "select", pin: false, default: "upper", choices: Object.fromEntries(Object.entries(TOOLS).map(([k, [l]]) => [k, l])), description: "What to do." },
    with: { type: "text", default: "", label: "With", description: "The text to look for, separator, count or pattern.", show: { tool: Object.keys(TOOLS).filter((k) => TOOLS[k][1]) } },
    replacement: { type: "text", default: "", label: "Replace with", show: { tool: ["replace"] }, description: "The text that goes where the found text was." },
  },
  outputs: {
    result: { type: "text", description: "The result." },
  },
  define({ values }) {
    const [label, , , type] = TOOLS[values.tool] || TOOLS.upper;
    return { outputs: { result: { type, label, description: `${label} of the text.` } } };
  },
  run({ fields }) {
    const t = String(fields.text ?? "");
    const w = String(fields.with ?? "");
    const result = {
      upper: () => t.toUpperCase(),
      lower: () => t.toLowerCase(),
      trim: () => t.trim(),
      length: () => t.length,
      contains: () => t.includes(w),
      starts: () => t.startsWith(w),
      ends: () => t.endsWith(w),
      replace: () => t.split(w).join(String(fields.replacement ?? "")),
      split: () => t.split(w || ","),
      words: () => t.split(/\s+/).filter(Boolean),
      slice: () => t.slice(0, Number(w) || 0),
      number: () => Number(t.replace(/[^\d.-]/g, "")) || 0,
      regex: () => new RegExp(w).test(t),
    }[fields.tool]();
    return { result };
  },
};
