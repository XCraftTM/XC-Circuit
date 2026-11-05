module.exports = {
  id: "discord.show_modal",
  kind: "action",
  info: {
    name: "Show Modal",
    description: "Opens a form: text boxes, menus, file uploads and text. It must be the first answer to a command, button or menu (no Send Message before it). Answers arrive at Modal Submitted.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "components",
    aliases: ["Form", "Popup"],
    tags: ["form", "popup", "dialog", "ask"],
  },
  look: {
    icon: "text",
  },
  fields: {
    interaction: { type: "discord.interaction", pin: "only", required: true, description: "The command use, button press or menu use to answer with the form." },
    customId: { type: "text", default: "my-form", label: "Custom id", description: "Identifies the form in Modal Submitted." },
    title: { type: "text", default: "Tell us more", description: "The form's title (up to 45 characters)." },
    inputs: {
      type: "list<discord.component>",
      pin: "only",
      multi: true,
      required: true,
      description: "Up to 5, in order: Text Inputs, Form Fields (a question with a Text Input, Select Menu or File Upload), Select Menus, File Uploads and Form Texts.",
    },
  },
  async run({ fields: f }) {
    const { ModalBuilder, ActionRowBuilder, LabelBuilder } = require("discord.js");
    const inputs = (f.inputs || []).flat().filter(Boolean).slice(0, 5);
    if (!inputs.length) throw new Error("A modal needs at least one input");
    const modal = new ModalBuilder().setCustomId(String(f.customId || "form").slice(0, 100)).setTitle(String(f.title || "Form").slice(0, 45));
    for (const input of inputs) {
      const kind = input?.constructor?.name;
      if (kind === "LabelBuilder") modal.addLabelComponents(input);
      else if (kind === "TextDisplayBuilder") modal.addTextDisplayComponents(input);
      else if (kind === "TextInputBuilder") modal.addActionRowComponents(new ActionRowBuilder().addComponents(input));
      else if (/SelectMenuBuilder$/.test(kind) || kind === "FileUploadBuilder") {
        // A menu or upload on its own gets a question: its placeholder, or a plain one.
        const label = new LabelBuilder().setLabel(String(input.data?.placeholder || (kind === "FileUploadBuilder" ? "Attach a file" : "Choose")).slice(0, 45));
        const put = {
          StringSelectMenuBuilder: "setStringSelectMenuComponent",
          UserSelectMenuBuilder: "setUserSelectMenuComponent",
          RoleSelectMenuBuilder: "setRoleSelectMenuComponent",
          ChannelSelectMenuBuilder: "setChannelSelectMenuComponent",
          MentionableSelectMenuBuilder: "setMentionableSelectMenuComponent",
          FileUploadBuilder: "setFileUploadComponent",
        }[kind];
        modal.addLabelComponents(label[put](input));
      } else {
        throw new Error("Inputs takes Text Inputs, Form Fields, Select Menus, File Uploads and Form Texts");
      }
    }
    await f.interaction.showModal(modal);
  },
};
