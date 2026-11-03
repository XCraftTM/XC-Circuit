module.exports = {
  id: "core.join_text",
  kind: "value",
  info: {
    name: "Join Text",
    description: "Joins several texts into one, with an optional separator between them.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "text",
    aliases: ["Concatenate", "Combine text"],
  },
  look: {
    icon: "text",
  },
  fields: {
    parts: { type: "text", multi: true, pin: "only", description: "The texts to join, in wire order." },
    separator: { type: "text", default: "", description: "Put between each part, such as a comma." },
  },
  outputs: {
    text: { type: "text", description: "All parts joined." },
  },
  run({ fields }) {
    return { text: (fields.parts || []).map((p) => String(p ?? "")).join(fields.separator ?? "") };
  },
};
