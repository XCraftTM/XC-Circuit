module.exports = {
  id: "fluxer.find_channel",
  kind: "action",
  info: {
    name: "Find Channel",
    description: "Finds a channel by its id or mention, or by its name in a server, and tells what it is.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["Get channel", "Channel by name", "Channel info"],
  },
  look: {
    icon: "hash",
  },
  fields: {
    channel: { type: "text", default: "", description: "Its id, <#id>, or its name (then connect the server)." },
    server: { type: "fluxer.server", pin: "only", description: "The server, to find a channel by name." },
  },
  outputs: {
    channel: { type: "fluxer.channel?", description: "The channel. Empty when not found." },
    found: { type: "boolean", description: "Whether there is such a channel." },
    name: { type: "text", description: "Its name." },
    id: { type: "text", description: "Its id." },
    mention: { type: "text", description: "<#id>, a link to the channel in a message." },
  },
  async run({ fields, ctx }) {
    const wanted = String(fields.channel ?? "").trim().replace(/^#/, "");
    if (!wanted) throw new Error("Type the channel's id or name");
    const byId = ctx.fluxer.id(wanted, "channel");
    let channel = null;
    if (byId) channel = await ctx.fluxer.client.channels.resolve(byId).catch(() => null);
    if (!channel && fields.server) {
      const match = (list) => [...list].find((c) => c.name?.toLowerCase() === wanted.toLowerCase());
      channel = match(fields.server.channels?.values?.() ?? []) ?? match(await fields.server.fetchChannels());
    }
    if (!channel && !byId && !fields.server) throw new Error("Connect the server to find a channel by name");
    if (!channel) return { channel: null, found: false, name: "", id: "", mention: "" };
    return { channel, found: true, name: channel.name ?? "", id: channel.id, mention: `<#${channel.id}>` };
  },
};
