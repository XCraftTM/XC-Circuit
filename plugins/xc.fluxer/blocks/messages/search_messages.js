module.exports = {
  id: "fluxer.search_messages",
  kind: "action",
  info: {
    name: "Search Messages",
    description: "Finds messages in a server by their text, channel or author, newest first.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "messages",
    aliases: ["Find messages", "Message search"],
  },
  look: {
    icon: "eye",
  },
  requires: {
    permissions: ["ReadMessageHistory"],
  },
  fields: {
    server: { type: "fluxer.server", pin: "only", required: true, description: "The server to search." },
    text: { type: "text", default: "", description: "Words the messages contain. Empty = any." },
    channel: { type: "any", pin: true, description: "Only in this channel (or its id)." },
    author: { type: "any", pin: true, description: "Only by this person (a user, member or id)." },
    limit: { type: "number", default: 25, min: 1, max: 100, step: 1, description: "At most this many." },
  },
  outputs: {
    messages: { type: "list<object>", description: "The matches, newest first." },
    count: { type: "number", description: "How many matched in all." },
  },
  async run({ fields: f, ctx }) {
    if (!f.server) throw new Error("Connect the server");
    const options = { contextGuildId: String(f.server.id), hitsPerPage: Math.min(100, Math.max(1, Math.floor(f.limit || 25))) };
    const text = String(f.text ?? "").trim();
    if (text) options.content = text;
    const channelId = f.channel ? ctx.fluxer.id(f.channel, "channel") : null;
    if (channelId) options.channelIds = [channelId];
    const authorId = f.author ? ctx.fluxer.id(f.author, "user") : null;
    if (authorId) options.authorIds = [authorId];
    const result = await ctx.fluxer.client.searchMessages(options);
    if (result?.indexing) throw new Error("Fluxer is still indexing this server's messages; try again in a while");
    const messages = result?.messages ?? [];
    return { messages, count: Number(result?.total ?? messages.length) };
  },
};
