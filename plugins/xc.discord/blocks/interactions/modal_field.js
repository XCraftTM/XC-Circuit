module.exports = {
  id: "discord.modal_field",
  kind: "value",
  info: {
    name: "Form Field",
    description: "Puts a question above a Text Input, Select Menu or File Upload in a form, with an optional hint under it. Connect it to Show Modal › Inputs.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "components",
    aliases: ["Label", "Modal field", "Form question", "Modal select", "Modal menu"],
    tags: ["form", "modal", "label", "components v2"],
  },
  look: {
    icon: "text",
  },
  fields: {
    label: { type: "text", default: "Your answer", description: "The question (up to 45 characters)." },
    description: { type: "text", default: "", label: "Hint", description: "Smaller text under the question (up to 100 characters). Empty = none." },
    input: {
      type: "discord.component",
      pin: "only",
      required: true,
      label: "Input",
      description: "A Text Input, a Select Menu (text, user, role, channel or mentionable) or a File Upload.",
    },
  },
  outputs: {
    field: { type: "discord.component", description: "Connect to Show Modal › Inputs." },
  },
  run({ fields: f }) {
    const { LabelBuilder } = require("discord.js");
    const label = new LabelBuilder().setLabel(String(f.label || "Answer").slice(0, 45));
    if (f.description) label.setDescription(String(f.description).slice(0, 100));
    const input = f.input;
    const kind = input?.constructor?.name;
    switch (kind) {
      case "TextInputBuilder":
        // The question is the field's; the box keeps no label of its own.
        if (input.data) delete input.data.label;
        label.setTextInputComponent(input);
        break;
      case "StringSelectMenuBuilder":
        label.setStringSelectMenuComponent(input);
        break;
      case "UserSelectMenuBuilder":
        label.setUserSelectMenuComponent(input);
        break;
      case "RoleSelectMenuBuilder":
        label.setRoleSelectMenuComponent(input);
        break;
      case "ChannelSelectMenuBuilder":
        label.setChannelSelectMenuComponent(input);
        break;
      case "MentionableSelectMenuBuilder":
        label.setMentionableSelectMenuComponent(input);
        break;
      case "FileUploadBuilder":
        label.setFileUploadComponent(input);
        break;
      default:
        throw new Error("Connect a Text Input, Select Menu or File Upload to Input");
    }
    return { field: label };
  },
};
