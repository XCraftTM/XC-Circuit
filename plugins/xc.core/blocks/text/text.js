module.exports = {
  id: "core.text",
  name: "Text",
  description: "A piece of text. Use {name} placeholders to fill in connected values.",
  author: "XC Team",
  version: "1.0.0",
  category: "text",
  icon: "text",
  kind: "value",
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
