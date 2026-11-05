const ACTIONS = { create: "Add an emoji", delete: "Delete an emoji", list: "List emojis", stickers: "List stickers" };

module.exports = {
  id: "discord.emojis",
  kind: "action",
  info: {
    name: "Server Emojis",
    description: "Adds or deletes a server's custom emojis, or lists its emojis and stickers. The bot needs Manage Expressions to change them.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Stickers", "Custom emoji"],
    tags: ["emoji", "emote", "sticker", "upload"],
  },
  look: {
    icon: "message",
  },
  requires: {
    permissions: {
      action: {
        create: ["ManageGuildExpressions"],
        delete: ["ManageGuildExpressions"],
      },
    },
  },
  fields: {
    action: { type: "select", pin: false, default: "list", choices: ACTIONS, description: "What to do." },
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    name: { type: "text", default: "", show: { action: ["create", "delete"] }, description: "The emoji's name (letters, numbers and _)." },
    image: { type: "text", default: "", show: { action: ["create"] }, label: "Image URL", description: "A PNG, JPG or GIF under 256 KB." },
  },
  outputs: {
    emoji: { type: "text", description: "The added emoji, ready for messages." },
    list: { type: "list<object>", description: "For lists: each name, id and text (or sticker url)." },
  },
  async run({ fields: f }) {
    const s = f.server;
    if (f.action === "create") {
      const e = await s.emojis.create({ attachment: f.image, name: String(f.name).replace(/[^\w]/g, "_").slice(0, 32) });
      return { emoji: e.toString(), list: [] };
    }
    if (f.action === "delete") {
      const e = s.emojis.cache.find((x) => x.name === String(f.name).replace(/:/g, ""));
      if (!e) throw new Error(`No emoji named ${f.name}`);
      await e.delete();
      return { emoji: "", list: [] };
    }
    if (f.action === "stickers") {
      const all = await s.stickers.fetch();
      return { emoji: "", list: [...all.values()].map((x) => ({ name: x.name, id: x.id, url: x.url })) };
    }
    const all = await s.emojis.fetch();
    return { emoji: "", list: [...all.values()].map((x) => ({ name: x.name, id: x.id, text: x.toString() })) };
  },
};
