const MODES = {
  Interaction: { reply: "Reply", edit: "Edit reply", followup: "Follow-up", defer: "Defer (think…)" },
  Message: { send: "Send to channel", msgReply: "Reply to message", msgEdit: "Edit message" },
};
const INTERACTION_MODES = ["reply", "edit", "followup", "defer"];

module.exports = {
  id: "discord.send_message",
  name: "Send Message",
  description: "Replies to a command or button, or sends, replies to and edits messages in channels.",
  author: "XC Team",
  version: "3.0.0",
  category: "messages",
  icon: "send",
  kind: "action",
  fields: {
    mode: { type: "select", pin: false, default: "reply", choices: MODES, description: "What kind of message to send." },
    text: { type: "textarea", template: true, default: "", description: "The message text. Use {name} for connected values." },
    embeds: { type: "list<discord.embed>", pin: true, multi: true, description: "Embeds to attach." },
    components: {
      type: "list<discord.component>",
      pin: true,
      multi: true,
      advanced: true,
      description: "Buttons and select menus. Up to 5 per row; rows fill in order.",
    },
    private: {
      type: "boolean",
      pin: false,
      default: false,
      label: "Private reply",
      description: "Only the person who used the command sees it.",
      show: { mode: ["reply", "followup", "defer"] },
    },
    split: { type: "boolean", pin: false, default: false, advanced: true, label: "Split long text", description: "Send text over 2000 characters as several messages." },
  },
  outputs: {
    message: { type: "discord.message", description: "The message that was sent." },
  },
  define({ values }) {
    const mode = values.mode || "reply";
    const add = {};
    if (INTERACTION_MODES.includes(mode)) {
      add.interaction = { type: "discord.interaction", pin: "only", required: true, description: "The command use or button press to answer." };
    } else if (mode === "send") {
      add.channel = { type: "discord.channel", pin: "only", required: true, description: "Where to send the message." };
    } else {
      add.message = { type: "discord.message", pin: "only", required: true, description: "The message to reply to or edit." };
    }
    const hide = mode === "defer" ? ["text", "embeds", "split"] : [];
    return { add, hide };
  },
  async run({ fields, ctx }) {
    const flags = fields.private ? ["Ephemeral"] : undefined;
    const payload = { content: fields.text || undefined, embeds: fields.embeds?.length ? fields.embeds : undefined, flags };
    const parts = (fields.components || []).filter(Boolean);
    if (parts.length) {
      // Buttons share a row (up to 5); a select menu takes a row of its own.
      const { ActionRowBuilder } = require("discord.js");
      const rows = [];
      for (const c of parts) {
        const isSelect = typeof c.setMinValues === "function";
        const last = rows[rows.length - 1];
        if (!isSelect && last && !last.isSelect && last.items.length < 5) last.items.push(c);
        else rows.push({ isSelect, items: [c] });
      }
      payload.components = rows.slice(0, 5).map((r) => new ActionRowBuilder().addComponents(...r.items));
    }
    const target = fields.interaction ?? fields.channel ?? fields.message;
    const send = {
      reply: () => (target.deferred ? target.editReply(payload) : target.reply({ ...payload, withResponse: true })),
      edit: () => target.editReply(payload),
      followup: () => target.followUp(payload),
      defer: () => target.deferReply({ flags }),
      send: () => target.send(payload),
      msgReply: () => target.reply(payload),
      msgEdit: () => target.edit(payload),
    }[fields.mode];
    const result = await send();
    return { message: result?.resource?.message ?? result ?? null };
  },
};
