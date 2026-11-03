module.exports = {
  id: "discord.show_modal",
  kind: "action",
  info: {
    name: "Show Modal",
    description: "Opens a form with text inputs. It must be the first answer to a command, button or menu (no Send Message before it). Answers arrive at Modal Submitted.",
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
    inputs: { type: "list<discord.component>", pin: "only", multi: true, required: true, description: "Up to 5 Text Inputs, in order." },
  },
  async run({ fields: f }) {
    const { ModalBuilder, ActionRowBuilder } = require("discord.js");
    const inputs = (f.inputs || []).flat().filter(Boolean).slice(0, 5);
    if (!inputs.length) throw new Error("A modal needs at least one Text Input");
    const modal = new ModalBuilder().setCustomId(String(f.customId || "form").slice(0, 100)).setTitle(String(f.title || "Form").slice(0, 45));
    modal.addComponents(...inputs.map((i) => new ActionRowBuilder().addComponents(i)));
    await f.interaction.showModal(modal);
  },
};
