// Splits a text at a separator and gives its parts ready to use: the first,
// the last, one by number, everything after the first, and all of them.
// For commands typed in chat ("!kick @user being rude"), lists in a text
// ("red, green, blue") and anything people separate.

/** The parts of `text`: at `separator`, or at spaces when it is empty. */
function parts(text, separator, { trim = true, skipEmpty = true } = {}) {
  const t = String(text ?? "");
  let list = separator === "" || separator == null ? t.split(/\s+/) : t.split(String(separator));
  if (trim) list = list.map((p) => p.trim());
  if (skipEmpty) list = list.filter((p) => p !== "");
  return list;
}

/** Part `n`: 1 is the first, 2 the second; -1 the last, -2 the one before. */
function nth(list, n) {
  const i = Number(n);
  if (!Number.isInteger(i) || i === 0) return "";
  return (i > 0 ? list[i - 1] : list[list.length + i]) ?? "";
}

module.exports = {
  id: "core.split_text",
  kind: "value",
  info: {
    name: "Split Text",
    description: "Splits a text at a separator (spaces, a comma, | …) and gives the first part, the last, one by number, the rest after the first, and how many there are.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "text",
    aliases: ["Split", "Arguments", "First word", "Last word", "Separate"],
    tags: ["split", "arguments", "args", "separator", "first", "last", "words", "command"],
  },
  look: {
    icon: "text",
  },
  fields: {
    text: { type: "text", pin: true, default: "", description: "The text to split, such as a message's content." },
    separator: { type: "text", pin: false, template: false, default: "", label: "Separator", placeholder: "Spaces", description: "What the parts are separated by, such as , or |. Empty: spaces (several count as one)." },
    number: { type: "number", default: 2, step: 1, label: "Part number", description: "Which part Part gives: 1 is the first, 2 the second; -1 the last, -2 the one before it." },
    trim: { type: "boolean", pin: false, default: true, label: "Trim spaces", advanced: true, description: "Remove spaces around each part." },
    skipEmpty: { type: "boolean", pin: false, default: true, label: "Skip empty parts", advanced: true, description: "Leave out parts with nothing in them (two separators in a row)." },
  },
  outputs: {
    first: { type: "text", description: "The first part (empty when there is none)." },
    last: { type: "text", description: "The last part." },
    part: { type: "text", description: "The part with the number in Part number." },
    rest: { type: "text", label: "Rest", description: "Everything after the first part, joined again: for !kick @user being rude, “@user being rude”." },
    parts: { type: "list<text>", description: "Every part, as a list." },
    count: { type: "number", description: "How many parts there are." },
  },
  run({ fields: f }) {
    const list = parts(f.text, f.separator, { trim: f.trim !== false, skipEmpty: f.skipEmpty !== false });
    const joiner = f.separator === "" || f.separator == null ? " " : String(f.separator);
    return {
      first: list[0] ?? "",
      last: list[list.length - 1] ?? "",
      part: nth(list, f.number),
      rest: list.slice(1).join(joiner),
      parts: list,
      count: list.length,
    };
  },
};
