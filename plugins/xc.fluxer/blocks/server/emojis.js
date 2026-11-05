const ACTIONS = {
  listEmojis: "List emojis",
  createEmoji: "Add an emoji",
  deleteEmoji: "Delete an emoji",
  listStickers: "List stickers",
  createSticker: "Add a sticker",
  deleteSticker: "Delete a sticker",
};

module.exports = {
  id: "fluxer.emojis",
  kind: "action",
  info: {
    name: "Emojis and Stickers",
    description: "Lists, adds or deletes a server's own emojis and stickers. New ones are made from an image link.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Add emoji", "Create emoji", "Delete emoji", "Add sticker", "Create sticker", "Emoji list"],
  },
  look: {
    icon: "star",
  },
  requires: {
    permissions: {
      action: {
        createEmoji: ["ManageExpressions"],
        deleteEmoji: ["ManageExpressions"],
        createSticker: ["ManageExpressions"],
        deleteSticker: ["ManageExpressions"],
      },
    },
  },
  fields: {
    action: { type: "select", pin: false, default: "listEmojis", choices: ACTIONS, description: "What to do." },
    server: { type: "fluxer.server", pin: "only", required: true, description: "The server." },
    name: { type: "text", default: "", show: { action: ["createEmoji", "createSticker", "deleteEmoji", "deleteSticker"] }, description: "Its name. To delete: the name or id." },
    image: { type: "text", default: "", show: { action: ["createEmoji", "createSticker"] }, description: "A link to the image (PNG, GIF or WebP)." },
    tags: { type: "text", default: "", show: { action: ["createSticker"] }, description: "Words that find the sticker, with commas." },
    description: { type: "text", default: "", show: { action: ["createSticker"] }, description: "What it shows." },
    reason: { type: "text", default: "", advanced: true, description: "Why, for the audit log." },
  },
  outputs: {
    list: { type: "list<object>", description: "For lists: every emoji or sticker (id, name, url)." },
    names: { type: "list<text>", description: "For lists: their names." },
    item: { type: "object?", description: "The added emoji or sticker." },
    text: { type: "text", description: "For added emojis: the emoji to put in a message (<:name:id>)." },
  },
  async run({ fields: f, ctx }) {
    if (!f.server) throw new Error("Connect the server");
    const stickers = /Sticker/.test(f.action);
    const reason = String(f.reason ?? "").trim() || undefined;
    const name = String(f.name ?? "").trim();
    const fetchAll = () => (stickers ? f.server.fetchStickers() : f.server.fetchEmojis());
    const shape = (e) => ({ id: e.id, name: e.name, url: typeof e.url === "function" ? e.url() : e.url, animated: Boolean(e.animated) });
    if (f.action === "listEmojis" || f.action === "listStickers") {
      const list = (await fetchAll()).map(shape);
      return { list, names: list.map((e) => e.name), item: null, text: "" };
    }
    if (f.action === "deleteEmoji" || f.action === "deleteSticker") {
      if (!name) throw new Error("Give the name or id to delete");
      const found = (await fetchAll()).find((e) => e.id === name || e.name === name);
      if (!found) throw new Error(`No ${stickers ? "sticker" : "emoji"} named ${name}`);
      await found.delete(reason);
      return { list: [], names: [], item: null, text: "" };
    }
    if (!name) throw new Error("Give it a name");
    const image = await ctx.fluxer.imageData(f.image);
    if (!image) throw new Error("Give a link to the image");
    if (stickers) {
      const tags = String(f.tags ?? "").split(",").map((t) => t.trim()).filter(Boolean);
      const sticker = await f.server.createSticker({ name, image, description: String(f.description ?? "").trim() || null, tags, reason });
      return { list: [], names: [], item: sticker, text: "" };
    }
    const emoji = await f.server.createEmoji({ name, image, reason });
    return { list: [], names: [], item: emoji, text: `<${emoji.animated ? "a" : ""}:${emoji.name}:${emoji.id}>` };
  },
};
