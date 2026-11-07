const MODES = {
  Interaction: { reply: "Reply", edit: "Edit reply", followup: "Follow-up", defer: "Defer (think…)", update: "Update the message (button or menu)" },
  Message: { send: "Send to channel", msgReply: "Reply to message", msgEdit: "Edit message" },
  User: { dm: "Direct message" },
};
const INTERACTION_MODES = ["reply", "edit", "followup", "defer", "update"];
const FORMATS = { text: "Text and embeds", layout: "Layout (Components V2)" };
const MENTIONS = { all: "Everyone mentioned", users: "Only users", usersRoles: "Users and roles (not @everyone)", none: "Nobody" };

module.exports = {
  id: "discord.send_message",
  kind: "action",
  info: {
    name: "Send Message",
    description: "Replies to a command or button, or sends, replies to and edits messages in channels.",
    author: "XC Team",
    version: "3.4.0",
  },
  search: {
    category: "send",
    aliases: ["Reply", "Respond", "Send DM"],
  },
  look: {
    icon: "send",
  },
  requires: {
    permissions: {
      mode: {
        Message: ["ViewChannel", "SendMessages", "EmbedLinks", "AttachFiles"],
      },
    },
  },
  fields: {
    mode: { type: "select", pin: false, default: "reply", choices: MODES, description: "What kind of message to send." },
    format: {
      type: "select",
      pin: false,
      default: "text",
      choices: FORMATS,
      description: "Text and embeds, or a layout from Build Layout (Components V2: containers, sections, galleries and more).",
      show: { mode: ["reply", "edit", "followup", "update", "send", "msgReply", "msgEdit", "dm"] },
    },
    text: { type: "textarea", template: true, default: "", description: "The message text. ${…} holds JavaScript, such as ${user.username}." },
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
      default: false,
      label: "Private reply",
      description: "Only the person who used the command sees it.",
      show: { mode: ["reply", "followup", "defer"] },
    },
    split: { type: "boolean", default: false, advanced: true, label: "Split long text", description: "Send text over 2000 characters as several messages." },
    files: { type: "list<any>", pin: "only", multi: true, description: "Files to send: a picture from Build Canvas, a file path (Read, Write or Download File), a web address or attachments of other messages." },
    mentions: { type: "select", pin: false, default: "all", advanced: true, label: "Who gets pinged", choices: MENTIONS, description: "Which mentions in the text notify people." },
    replyPing: { type: "boolean", default: true, advanced: true, label: "Ping the author", show: { mode: ["msgReply"] }, description: "Notify the person being replied to." },
    silent: { type: "boolean", default: false, advanced: true, description: "Send without a notification sound (@silent)." },
    noEmbeds: { type: "boolean", default: false, advanced: true, label: "No link previews", description: "Don't show previews of links in the text." },
    tts: { type: "boolean", default: false, advanced: true, label: "Read aloud", description: "Text to speech, for people who have it on." },
    stickers: { type: "text", default: "", advanced: true, description: "Sticker ids to send, comma separated (up to 3)." },
    deleteAfter: { type: "number", default: 0, min: 0, advanced: true, label: "Delete after (s)", description: "Delete the sent message after this many seconds. 0 keeps it." },
  },
  outputs: {
    message: { type: "discord.message", description: "The message that was sent." },
  },
  define({ values }) {
    const mode = values.mode || "reply";
    const add = {};
    if (INTERACTION_MODES.includes(mode)) {
      add.interaction = { type: "discord.interaction", pin: "only", required: true, description: "The command use or button press to answer." };
    } else if (mode === "dm") {
      add.user = { type: "discord.user", pin: "only", required: true, description: "Who gets the direct message (a member works too)." };
    } else if (mode === "send") {
      add.channel = { type: "discord.channel", pin: "only", required: true, description: "Where to send the message." };
    } else {
      add.message = { type: "discord.message", pin: "only", required: true, description: "The message to reply to or edit." };
    }
    const hide = mode === "defer" ? ["text", "embeds", "split", "files"] : [];
    if (mode !== "defer" && values.format === "layout") {
      // A Components V2 message has no text or embeds of its own: they are in the layout.
      add.layout = { type: "discord.layout", pin: "only", required: true, description: "The message from Build Layout." };
      hide.push("text", "embeds", "split", "tts", "noEmbeds");
    }
    return { add, hide };
  },
  async run({ fields, ctx }) {
    const layout = fields.format === "layout" && fields.mode !== "defer" ? fields.layout || { components: [] } : null;
    const flagList = [fields.private && "Ephemeral", fields.silent && "SuppressNotifications", !layout && fields.noEmbeds && "SuppressEmbeds", layout && "IsComponentsV2"].filter(
      Boolean,
    );
    const flags = flagList.length ? flagList : undefined;
    // A drawn picture ({ attachment: <bytes>, name }) goes as it is, keeping its name.
    const files = (fields.files || []).flat().filter(Boolean).map((f) => (Buffer.isBuffer(f.attachment) ? f : f.url ?? f.attachment ?? f));
    const payload = { content: fields.text || undefined, embeds: fields.embeds?.length ? fields.embeds : undefined, flags, files: files.length ? files : undefined };
    if (fields.tts) payload.tts = true;
    const stickers = String(fields.stickers || "").split(",").map((s) => s.trim()).filter(Boolean);
    if (stickers.length) payload.stickers = stickers.slice(0, 3);
    const parse = { users: ["users"], usersRoles: ["users", "roles"], none: [] }[fields.mentions];
    if (parse || fields.replyPing === false) payload.allowedMentions = { ...(parse ? { parse } : {}), repliedUser: fields.replyPing !== false };
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
    if (layout) {
      // Components V2: the layout is the whole message; buttons and menus from
      // the Components input follow it in rows of their own.
      Object.assign(payload, { content: undefined, embeds: undefined, tts: undefined });
      payload.components = [...(layout.components || []), ...(payload.components || [])];
      const all = [...(layout.files || []), ...(payload.files || [])];
      payload.files = all.length ? all : undefined;
    }
    const target = fields.interaction ?? fields.channel ?? fields.message ?? (fields.user?.user ?? fields.user);
    const send = {
      reply: () => (target.deferred ? target.editReply(payload) : target.reply({ ...payload, withResponse: true })),
      edit: () => target.editReply(payload),
      followup: () => target.followUp(payload),
      defer: () => target.deferReply({ flags }),
      update: () => target.update({ ...payload, flags: layout ? ["IsComponentsV2"] : undefined, withResponse: true }),
      dm: () => target.send(payload),
      send: () => target.send(payload),
      msgReply: () => target.reply(payload),
      msgEdit: () => target.edit(payload),
    }[fields.mode];
    const result = await send();
    const message = result?.resource?.message ?? (typeof result?.delete === "function" ? result : null);
    if (message && fields.deleteAfter > 0) setTimeout(() => message.delete().catch(() => {}), fields.deleteAfter * 1000);
    return { message };
  },
};
