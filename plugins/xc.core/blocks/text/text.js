module.exports = {
  id: "core.text",
  kind: "value",
  info: {
    name: "Text",
    description: "A piece of text. ${…} holds JavaScript, such as ${count * 2}.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "text",
    aliases: ["String"],
  },
  look: {
    icon: "text",
  },
  fields: {
    text: { type: "textarea", pin: false, template: true, default: "", description: "The text." },
  },
  outputs: {
    text: { type: "text", description: "The finished text." },
  },
  run({ fields }) {
    return { text: String(fields.text ?? "") };
  },
};
