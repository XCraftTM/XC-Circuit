// A form (Discord calls it a modal), designed in the Modal Builder and kept
// as Discord's own JSON. ${…} in its texts holds JavaScript. Forms made the
// older way, from Text Input, Form Field, File Upload and Form Text blocks
// wired into Inputs, still work.

/** A copy of a JSON value with `fn` applied to every string. */
function mapStrings(v, fn) {
  if (typeof v === "string") return fn(v);
  if (Array.isArray(v)) return v.map((x) => mapStrings(x, fn));
  if (v && typeof v === "object") return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, mapStrings(x, fn)]));
  return v;
}

/** The older way: builders from the blocks wired into Inputs. */
function fromBlocks(inputs, f) {
  const { ModalBuilder, ActionRowBuilder, LabelBuilder } = require("discord.js");
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
  return modal;
}

module.exports = {
  id: "discord.show_modal",
  kind: "action",
  info: {
    name: "Show Modal",
    description:
      "Opens a form designed in the Modal Builder: questions with a box to type into, a select menu or a file upload, and texts. It must be the first answer to a command, button or menu (no Send Message before it). Answers arrive at Modal Submitted.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "components",
    aliases: ["Form", "Popup", "Modal Builder"],
    tags: ["form", "popup", "dialog", "ask", "question"],
  },
  look: {
    icon: "text",
    width: 300,
  },
  fields: {
    interaction: { type: "discord.interaction", pin: "only", required: true, description: "The command use, button press or menu use to answer with the form." },
    form: {
      type: "modal",
      pin: false,
      // Names used in ${…} in the form's texts get an input, unless earlier blocks provide them.
      template: true,
      label: "Form",
      default: {
        title: "Tell us more",
        custom_id: "my-form",
        components: [{ type: 18, label: "Your answer", component: { type: 4, custom_id: "answer", style: 1 } }],
      },
      description: "The form: its title, its custom id (what Modal Submitted matches) and up to 5 questions and texts. Open the Modal Builder to edit it.",
    },
    customId: { type: "text", default: "my-form", label: "Custom id", description: "Identifies the form in Modal Submitted (with blocks wired into Inputs)." },
    title: { type: "text", default: "Tell us more", description: "The form's title, up to 45 characters (with blocks wired into Inputs)." },
    inputs: {
      type: "list<discord.component>",
      pin: "only",
      multi: true,
      advanced: true,
      description: "The older way to make a form: up to 5 Text Inputs, Form Fields, Select Menus, File Uploads and Form Texts, in order. When wired, they are used instead of the Form.",
    },
  },
  define({ connected }) {
    // Blocks wired into Inputs make the form; otherwise the Modal Builder's form does.
    return connected.inputs ? { hide: ["form"] } : { hide: ["customId", "title"] };
  },
  async run({ fields: f, ctx }) {
    const inputs = (f.inputs || []).flat().filter(Boolean).slice(0, 5);
    if (inputs.length) {
      await f.interaction.showModal(fromBlocks(inputs, f));
      return;
    }
    const { ModalBuilder } = require("discord.js");
    const form = mapStrings(f.form || {}, (s) => (ctx.fill ? ctx.fill(s, f) : s));
    if (!Array.isArray(form.components) || !form.components.length) throw new Error("The form has no questions: open the Modal Builder to add some");
    form.title = String(form.title || "Form").slice(0, 45);
    form.custom_id = String(form.custom_id || "form").slice(0, 100);
    await f.interaction.showModal(new ModalBuilder(form));
  },
};
