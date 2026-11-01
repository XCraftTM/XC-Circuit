module.exports = {
  id: "discord.find_channel",
  name: "Find Channel",
  description: "Looks up a channel by id, or by name inside a server.",
  author: "XC Team",
  version: "1.0.0",
  category: "channels",
  icon: "hash",
  kind: "action",
  tags: ["fetch", "get", "lookup"],
  fields: {
    channel: { type: "text", required: true, default: "", label: "Id or name", description: "A channel id, or a name such as welcome." },
    server: { type: "discord.guild?", pin: true, description: "Needed when looking up by name." },
  },
  outputs: {
    channel: { type: "discord.channel", description: "The channel. Fails when it is not found." },
  },
  async run({ fields, ctx }) {
    const key = String(fields.channel).replace(/^#/, "");
    if (/^\d{15,}$/.test(key)) return { channel: await ctx.discord.client.channels.fetch(key) };
    const found = fields.server?.channels.cache.find((c) => c.name === key);
    if (!found) throw new Error(`No channel named "${key}"${fields.server ? "" : " (connect a server to look up by name)"}`);
    return { channel: found };
  },
};
