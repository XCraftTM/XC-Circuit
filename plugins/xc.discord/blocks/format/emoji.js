module.exports = {
  id: "discord.emoji",
  name: "Emoji",
  description: "A server's custom emoji by name, as text to put in messages and reactions.",
  author: "XC Team",
  version: "1.0.0",
  category: "format",
  icon: "message",
  kind: "value",
  tags: ["custom emoji", "emote"],
  fields: {
    name: { type: "text", required: true, default: "", description: "The emoji's name, without colons." },
    server: { type: "discord.guild?", pin: "only", description: "Look only in this server. Empty = every server the bot is in." },
  },
  outputs: {
    text: { type: "text", description: "The emoji, such as <:wave:123…>. Empty when not found." },
    found: { type: "boolean", description: "Whether it was found." },
  },
  run({ fields, ctx }) {
    const name = String(fields.name).replace(/:/g, "");
    const pool = fields.server ? fields.server.emojis.cache : ctx.discord.client.emojis.cache;
    const e = pool.find((x) => x.name === name);
    return { text: e ? e.toString() : "", found: Boolean(e) };
  },
};
