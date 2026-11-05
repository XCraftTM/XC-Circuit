const ACTIONS = { send: "Send a sticker", list: "List the server's stickers", create: "Add a sticker", delete: "Delete a sticker" };

/** A server sticker by id or name. */
async function findSticker(server, which) {
  const key = String(which || "").trim();
  if (!key) throw new Error("Say which sticker (its name or id)");
  const all = await server.stickers.fetch();
  const sticker = all.get(key) ?? all.find((s) => s.name.toLowerCase() === key.toLowerCase());
  if (!sticker) throw new Error(`No sticker named ${key}`);
  return sticker;
}

module.exports = {
  id: "discord.stickers",
  kind: "action",
  info: {
    name: "Stickers",
    description: "Sends a server sticker in a channel (or as a reply), lists a server's stickers, or adds and deletes them. Adding needs Manage Expressions and a PNG, APNG, GIF or Lottie JSON file under 512 KB.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "send",
    aliases: ["Send sticker", "Add sticker", "Delete sticker"],
    tags: ["sticker", "expression", "emoji", "upload"],
  },
  look: {
    icon: "message",
  },
  requires: {
    permissions: {
      action: {
        send: ["SendMessages", "UseExternalStickers"],
        create: ["ManageGuildExpressions"],
        delete: ["ManageGuildExpressions"],
      },
    },
  },
  fields: {
    action: { type: "select", pin: false, default: "send", choices: ACTIONS, description: "What to do." },
    server: { type: "discord.guild", pin: "only", required: true, description: "The server whose stickers to use." },
    channel: { type: "discord.channel?", pin: "only", show: { action: ["send"] }, description: "Where to send it (or connect Reply to)." },
    replyTo: { type: "discord.message?", pin: "only", show: { action: ["send"] }, label: "Reply to", description: "Sends it as a reply to this message instead." },
    sticker: { type: "text", default: "", show: { action: ["send", "delete"] }, description: "The sticker's name or id." },
    text: { type: "text", default: "", show: { action: ["send"] }, description: "Text to send with it (optional)." },
    name: { type: "text", default: "", show: { action: ["create"] }, description: "The new sticker's name (2 to 30 characters)." },
    file: { type: "text", default: "", show: { action: ["create"] }, label: "File URL", description: "The picture: a URL or a file path." },
    emoji: { type: "text", default: "🙂", show: { action: ["create"] }, label: "Related emoji", description: "The emoji it stands for, used for suggestions." },
    description: { type: "text", default: "", show: { action: ["create"] }, description: "A short description (optional)." },
  },
  outputs: {
    message: { type: "discord.message?", description: "For send: the sent message." },
    sticker: { type: "object", description: "The sent, added or found sticker." },
    list: { type: "list<object>", description: "For lists: each name, id, url and emoji." },
  },
  async run({ fields: f }) {
    const s = f.server;
    if (!s?.stickers) throw new Error("Connect a server");
    const none = { message: null, sticker: null, list: [] };
    if (f.action === "list") {
      const all = await s.stickers.fetch();
      return { ...none, list: [...all.values()].map((x) => ({ name: x.name, id: x.id, url: x.url, emoji: x.tags ?? "" })) };
    }
    if (f.action === "create") {
      const sticker = await s.stickers.create({ file: f.file, name: String(f.name).slice(0, 30), tags: String(f.emoji || "🙂"), description: f.description || undefined });
      return { ...none, sticker };
    }
    const sticker = await findSticker(s, f.sticker);
    if (f.action === "delete") {
      await sticker.delete();
      return none;
    }
    const payload = { stickers: [sticker.id] };
    if (f.text) payload.content = String(f.text);
    const message = f.replyTo ? await f.replyTo.reply(payload) : await f.channel.send(payload);
    return { ...none, message, sticker };
  },
};
