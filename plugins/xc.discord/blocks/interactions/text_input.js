const STYLES = { short: "One line", paragraph: "Several lines" };

module.exports = {
  id: "discord.text_input",
  kind: "value",
  info: {
    name: "Text Input",
    description: "A box to type into, for Show Modal. Its answer comes out of Modal Submitted under its id.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "components",
    aliases: ["Form field"],
    tags: ["form", "modal", "field", "question"],
  },
  look: {
    icon: "text",
  },
  fields: {
    inputId: { type: "text", default: "answer", label: "Input id", description: "The answer's name in Modal Submitted." },
    label: { type: "text", default: "Your answer", description: "The question above the box (up to 45 characters)." },
    style: { type: "select", pin: false, default: "short", choices: STYLES, description: "One line, or a bigger box." },
    placeholder: { type: "text", default: "", description: "Gray hint text inside the empty box." },
    value: { type: "text", default: "", label: "Starts with", advanced: true, description: "Text already in the box." },
    required: { type: "boolean", default: true, description: "Whether it must be filled in." },
    min: { type: "number", default: 0, min: 0, max: 4000, step: 1, label: "At least (characters)", advanced: true, description: "0 = no minimum." },
    max: { type: "number", default: 4000, min: 1, max: 4000, step: 1, label: "At most (characters)", advanced: true, description: "Up to 4000." },
  },
  outputs: {
    input: { type: "discord.component", description: "Connect to Show Modal › Inputs." },
  },
  run({ fields: f }) {
    const { TextInputBuilder, TextInputStyle } = require("discord.js");
    const t = new TextInputBuilder()
      .setCustomId(String(f.inputId || "answer").slice(0, 100))
      .setLabel(String(f.label || "Answer").slice(0, 45))
      .setStyle(f.style === "paragraph" ? TextInputStyle.Paragraph : TextInputStyle.Short)
      .setRequired(Boolean(f.required));
    if (f.placeholder) t.setPlaceholder(String(f.placeholder).slice(0, 100));
    if (f.value) t.setValue(String(f.value).slice(0, 4000));
    if (f.min > 0) t.setMinLength(Math.floor(f.min));
    if (f.max) t.setMaxLength(Math.min(4000, Math.floor(f.max)));
    return { input: t };
  },
};
