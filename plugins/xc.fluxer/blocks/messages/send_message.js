const MODES = { send: "Send to channel", reply: "Reply to message", edit: "Edit message", dm: "Direct message" };

module.exports = {
  id: "fluxer.send_message",
  kind: "action",
  info: {
    name: "Send Message",
    description: "Sends a message to a channel or a person, replies to a message, or edits one, with text and embeds.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "send",
    aliases: ["Reply", "Respond", "Send DM", "Edit message"],
  },
  look: {
    icon: "send",
  },
  fields: {
    mode: { type: "select", pin: false, default: "reply", choices: MODES, description: "What kind of message to send." },
    text: { type: "textarea", template: true, default: "", description: "The message text. ${…} holds JavaScript, such as ${author.username}." },
    embeds: { type: "list<fluxer.embed>", pin: true, multi: true, description: "Embeds to attach, from Build Embed." },
    ping: { type: "boolean", default: true, advanced: true, label: "Ping the author", show: { mode: ["reply"] }, description: "Notify the person being replied to." },
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
    return { add };
  },
  async run({ fields, ctx }) {
    const embeds = [fields.embeds].flat().filter(Boolean);
    const content = String(fields.text ?? "");
    if (!content.trim() && !embeds.length && fields.mode !== "edit") throw new Error("Nothing to send: add text or an embed");
    const payload = { content: content || undefined, embeds: embeds.length ? embeds : undefined };
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
        sent = await fields.message.reply(payload, { ping: fields.ping !== false });
    }
    const seconds = Number(fields.deleteAfter) || 0;
    if (seconds > 0 && sent) {
      setTimeout(() => sent.delete().catch(() => {}), seconds * 1000).unref?.();
    }
    return { message: sent };
  },
};
