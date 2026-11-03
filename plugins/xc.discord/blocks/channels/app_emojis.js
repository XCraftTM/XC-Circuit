const ACTIONS = { list: "List the bot's emojis", create: "Add an emoji", delete: "Delete an emoji", find: "Find an emoji by name" };

module.exports = {
  id: "discord.app_emojis",
  kind: "action",
  info: {
    name: "Bot Emojis",
    description: "The bot's own emojis (app emojis): usable in every server and DM, no matter where the bot is. Add up to 2000 in the developer portal or here.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "bot",
    aliases: ["App emojis", "Application emojis", "Bot emoji"],
    tags: ["emoji", "emote", "application", "upload"],
  },
  look: {
    icon: "message",
  },
  fields: {
    action: { type: "select", pin: false, default: "find", choices: ACTIONS, description: "What to do." },
    name: { type: "text", default: "", show: { action: ["create", "delete", "find"] }, description: "The emoji's name (letters, numbers and _)." },
    image: { type: "text", default: "", show: { action: ["create"] }, label: "Image URL", description: "A PNG, JPG, GIF or WebP under 256 KB." },
  },
  outputs: {
    emoji: { type: "text", description: "The emoji, ready for messages, such as <:wave:123>." },
    id: { type: "text", description: "Its id." },
    list: { type: "list<object>", description: "For lists: each name, id and text." },
  },
  async run({ fields: f, ctx }) {
    const app = ctx.discord.client.application;
    const name = String(f.name || "").replace(/:/g, "").trim();
    if (f.action === "create") {
      const e = await app.emojis.create({ attachment: f.image, name: name.replace(/[^\w]/g, "_").slice(0, 32) });
      return { emoji: e.toString(), id: e.id, list: [] };
    }
    const all = await app.emojis.fetch();
    if (f.action === "list") return { emoji: "", id: "", list: [...all.values()].map((e) => ({ name: e.name, id: e.id, text: e.toString() })) };
    const e = all.find((x) => x.name === name || x.id === name);
    if (!e) throw new Error(`The bot has no emoji named ${name}`);
    if (f.action === "delete") {
      await e.delete();
      return { emoji: "", id: e.id, list: [] };
    }
    return { emoji: e.toString(), id: e.id, list: [] };
  },
};
