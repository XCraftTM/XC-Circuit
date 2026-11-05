module.exports = {
  id: "discord.modal_text",
  kind: "value",
  info: {
    name: "Form Text",
    description: "Text shown in a form between its fields: instructions, rules, a heading. Discord's text formatting works (**bold**, # headings, lists).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "components",
    aliases: ["Text display", "Modal text", "Form instructions"],
    tags: ["form", "modal", "text", "components v2"],
  },
  look: {
    icon: "text",
  },
  fields: {
    text: { type: "textarea", default: "Please fill in every field.", description: "What the form shows (up to 4000 characters)." },
  },
  outputs: {
    text: { type: "discord.component", label: "Form text", description: "Connect to Show Modal › Inputs." },
  },
  run({ fields: f }) {
    const { TextDisplayBuilder } = require("discord.js");
    const content = String(f.text ?? "").trim();
    if (!content) throw new Error("Form Text needs some text");
    return { text: new TextDisplayBuilder().setContent(content.slice(0, 4000)) };
  },
};
