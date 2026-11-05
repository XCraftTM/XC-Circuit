module.exports = {
  id: "discord.file_upload",
  kind: "value",
  info: {
    name: "File Upload",
    description: "A place in a form where people attach files. Put it in a Form Field (or connect it to Show Modal directly). The files arrive at Modal Submitted › Files.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "components",
    aliases: ["Upload", "Attachment", "File input", "Modal file"],
    tags: ["form", "modal", "file", "components v2"],
  },
  look: {
    icon: "folder-plus",
  },
  fields: {
    inputId: { type: "text", default: "files", label: "Input id", description: "The answer's name in Modal Submitted." },
    min: { type: "number", default: 1, min: 0, max: 10, step: 1, label: "At least (files)", description: "0 to 10." },
    max: { type: "number", default: 1, min: 1, max: 10, step: 1, label: "At most (files)", description: "1 to 10." },
    required: { type: "boolean", default: true, description: "Whether a file must be attached." },
  },
  outputs: {
    input: { type: "discord.component", description: "Connect to Form Field › Input, or Show Modal › Inputs." },
  },
  run({ fields: f }) {
    const { FileUploadBuilder } = require("discord.js");
    const max = Math.min(10, Math.max(1, Math.floor(f.max || 1)));
    const min = Math.min(max, Math.max(0, Math.floor(f.min ?? 1)));
    const upload = new FileUploadBuilder()
      .setCustomId(String(f.inputId || "files").slice(0, 100))
      .setMinValues(min)
      .setMaxValues(max)
      .setRequired(Boolean(f.required));
    return { input: upload };
  },
};
