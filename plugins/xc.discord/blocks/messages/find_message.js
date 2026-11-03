module.exports = {
  id: "discord.find_message",
  kind: "action",
  info: {
    name: "Find Message",
    description: "Looks up a message by its id (in a channel) or by its link.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "messages",
    tags: ["fetch", "get", "lookup", "link"],
  },
  look: {
    icon: "message",
  },
  fields: {
    message: { type: "text", required: true, default: "", label: "Id or link", description: "A message id, or a link such as https://discord.com/channels/…/…/…" },
    channel: { type: "discord.channel?", pin: "only", description: "Where it is (not needed for a link)." },
  },
  outputs: {
    message: { type: "discord.message", description: "The message. Fails when it isn't found." },
  },
  async run({ fields, ctx }) {
    const text = String(fields.message).trim();
    const link = /channels\/(?:\d+|@me)\/(\d+)\/(\d+)/.exec(text);
    const channel = link ? await ctx.discord.client.channels.fetch(link[1]) : fields.channel;
    if (!channel) throw new Error("Connect the channel the message is in, or use its link");
    return { message: await channel.messages.fetch(link ? link[2] : text) };
  },
};
