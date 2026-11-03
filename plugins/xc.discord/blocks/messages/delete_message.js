module.exports = {
  id: "discord.delete_message",
  kind: "action",
  info: {
    name: "Delete Message",
    description: "Deletes a message, optionally after a delay.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "messages",
    aliases: ["Remove message"],
    tags: ["remove"],
  },
  look: {
    icon: "message",
  },
  fields: {
    message: { type: "discord.message", pin: "only", required: true, description: "The message to delete." },
    afterSeconds: { type: "number", default: 0, min: 0, label: "After (s)", description: "Wait this long first. 0 = now." },
  },
  async run({ fields, ctx }) {
    if (fields.afterSeconds > 0) await ctx.util.sleep(fields.afterSeconds * 1000);
    await fields.message.delete();
  },
};
