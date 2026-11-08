module.exports = {
  id: "discord.find_channel",
  kind: "action",
  info: {
    name: "Find Channel",
    description: "Looks up a channel by id, or by name inside a server.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "channels",
    aliases: ["Get channel"],
    tags: ["fetch", "get", "lookup"],
  },
  look: {
    icon: "hash",
  },
  flows: ["next", "missing"],
  fields: {
    channel: { type: "text", required: true, default: "", label: "Id or name", description: "A channel id, or a name such as welcome." },
    server: { type: "discord.guild?", pin: true, description: "Needed when looking up by name." },
  },
  outputs: {
    channel: { type: "discord.channel", description: "The channel. When it isn't found, Missing runs instead (or, with nothing connected to Missing, the block fails)." },
  },
  async run({ fields, ctx }) {
    const key = String(fields.channel).trim().replace(/^<#(\d+)>$/, "$1").replace(/^#/, "");
    const found = /^\d{15,}$/.test(key) ? await ctx.discord.client.channels.fetch(key).catch(() => null) : fields.server?.channels.cache.find((c) => c.name === key);
    if (!found) {
      if (ctx.wired?.("missing")) return ctx.next("missing", { channel: null });
      throw new Error(`No channel ${/^\d+$/.test(key) ? "with the id" : "named"} "${key}"${fields.server || /^\d+$/.test(key) ? "" : " (connect a server to look up by name)"}`);
    }
    await ctx.next("next", { channel: found });
  },
};
