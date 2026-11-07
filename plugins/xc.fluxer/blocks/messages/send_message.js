const MODES = { send: "Send to channel", reply: "Reply to message", edit: "Edit message", dm: "Direct message" };

/**
 * A file as @fluxerjs/core takes it: a drawn picture ({ attachment, name },
 * from Build Canvas), a web address, a file path, or an attachment of
 * another message.
 */
function fileOf(f) {
  const nameOf = (s) => decodeURIComponent(String(s).split(/[?#]/)[0].split(/[\\/]/).pop() || "") || "file";
  if (f && Buffer.isBuffer(f.attachment)) return { name: f.name || "file", data: f.attachment };
  if (Buffer.isBuffer(f)) return { name: "file", data: f };
  if (f && typeof f === "object" && f.url) return { name: f.filename || f.name || nameOf(f.url), url: f.url };
  const s = String(f ?? "").trim();
  if (/^https?:\/\//i.test(s)) return { name: nameOf(s), url: s };
  return { name: nameOf(s), data: require("fs").readFileSync(s) };
}

module.exports = {
  id: "fluxer.send_message",
  kind: "action",
  info: {
    name: "Send Message",
    description: "Sends a message to a channel or a person, replies to a message, or edits one, with text, embeds and files.",
    author: "XC Team",
    version: "1.2.0",
  },
  search: {
    category: "send",
    aliases: ["Reply", "Respond", "Send DM", "Edit message"],
  },
  look: {
    icon: "send",
  },
  requires: {
    permissions: {
      mode: {
        send: ["ViewChannel", "SendMessages", "EmbedLinks", "AttachFiles"],
        reply: ["SendMessages", "ReadMessageHistory"],
      },
    },
  },
  fields: {
    mode: { type: "select", pin: false, default: "reply", choices: MODES, description: "What kind of message to send." },
    text: { type: "textarea", template: true, default: "", description: "The message text. ${…} holds JavaScript, such as ${author.username}." },
    embeds: { type: "list<fluxer.embed>", pin: true, multi: true, description: "Embeds to attach, from Build Embed." },
    files: { type: "list<any>", pin: "only", multi: true, description: "Files to send: a picture from Build Canvas, web addresses, file paths or attachments of other messages." },
    // Not "ping": a ${ping} in the text (Bot Info's ping) would read this switch.
    pingAuthor: { type: "boolean", default: true, advanced: true, label: "Ping the author", show: { mode: ["reply"] }, description: "Notify the person being replied to." },
    deleteAfter: { type: "number", default: 0, min: 0, advanced: true, label: "Delete after (s)", description: "Delete the sent message after this many seconds. 0 keeps it." },
  },
  outputs: {
    message: { type: "fluxer.message", description: "The message that was sent or edited." },
  },
  define({ values }) {
    const mode = values.mode || "reply";
    const add = {};
    if (mode === "send") add.channel = { type: "fluxer.channel", pin: "only", required: true, description: "Where to send the message." };
    else if (mode === "dm") add.user = { type: "fluxer.user", pin: "only", required: true, description: "Who gets the direct message (a member works too)." };
    else add.message = { type: "fluxer.message", pin: "only", required: true, description: mode === "edit" ? "The message to edit (the bot's own)." : "The message to reply to." };
    // Fluxer can't add files to a message that was sent.
    return { add, hide: mode === "edit" ? ["files"] : [] };
  },
  async run({ fields, ctx }) {
    const embeds = [fields.embeds].flat().filter(Boolean);
    const files = [fields.files].flat(2).filter(Boolean).map(fileOf);
    const content = String(fields.text ?? "");
    if (!content.trim() && !embeds.length && !files.length && fields.mode !== "edit") throw new Error("Nothing to send: add text, an embed or a file");
    const payload = { content: content || undefined, embeds: embeds.length ? embeds : undefined, files: files.length ? files : undefined };
    let sent;
    switch (fields.mode) {
      case "send":
        sent = await (await ctx.fluxer.channel(fields.channel)).send(payload);
        break;
      case "dm":
        sent = await (await ctx.fluxer.user(fields.user)).send(payload);
        break;
      case "edit":
        if (!fields.message) throw new Error("Connect the message to edit");
        sent = await fields.message.edit({ content, embeds });
        break;
      default:
        if (!fields.message) throw new Error("Connect the message to reply to");
        sent = await fields.message.reply(payload, { ping: fields.pingAuthor !== false });
    }
    const seconds = Number(fields.deleteAfter) || 0;
    if (seconds > 0 && sent) {
      setTimeout(() => sent.delete().catch(() => {}), seconds * 1000).unref?.();
    }
    return { message: sent };
  },
};
