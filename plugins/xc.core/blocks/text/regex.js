const MODES = { test: "Does it match?", first: "First match", all: "All matches", replace: "Replace matches" };

module.exports = {
  id: "core.regex",
  kind: "value",
  info: {
    name: "Regex",
    description: "Finds patterns in text with a regular expression: test, get matches and their groups, or replace.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "text",
    aliases: ["Regular expression", "Pattern"],
    tags: ["pattern", "match", "regular expression", "extract", "find"],
  },
  look: {
    icon: "text",
  },
  fields: {
    text: { type: "text", pin: true, default: "", description: "The text to search." },
    pattern: { type: "text", template: false, default: "\\d+", description: "The regular expression, such as \\d+ or (\\w+)@(\\w+)." },
    mode: { type: "select", pin: false, default: "first", choices: MODES, description: "What to do." },
    replacement: { type: "text", template: false, default: "", show: { mode: ["replace"] }, description: "What matches become. $1 inserts the first group." },
    ignoreCase: { type: "boolean", default: true, label: "Ignore upper/lower case", description: "Treat A and a the same." },
  },
  outputs: {
    result: { type: "any", description: "True/false, the match, the list of matches, or the new text." },
    groups: { type: "list<text>", description: "The first match's groups (the parts in brackets)." },
    matched: { type: "boolean", description: "Whether anything matched." },
  },
  define({ values }) {
    const type = { test: "boolean", first: "text", all: "list<text>", replace: "text" }[values.mode] || "any";
    return { outputs: { result: { type, description: "The result." } } };
  },
  run({ fields: f }) {
    const flags = f.ignoreCase ? "i" : "";
    const text = String(f.text ?? "");
    const re = new RegExp(String(f.pattern), flags);
    const first = re.exec(text);
    const groups = first ? first.slice(1).map((g) => g ?? "") : [];
    const result = {
      test: () => Boolean(first),
      first: () => (first ? first[0] : ""),
      all: () => [...text.matchAll(new RegExp(String(f.pattern), flags + "g"))].map((m) => m[0]),
      replace: () => text.replace(new RegExp(String(f.pattern), flags + "g"), String(f.replacement ?? "")),
    }[f.mode]();
    return { result, groups, matched: Boolean(first) };
  },
};
