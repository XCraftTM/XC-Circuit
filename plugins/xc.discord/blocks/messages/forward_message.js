module.exports = {
  id: "discord.forward_message",
  kind: "action",
  info: {
    name: "Forward Message",
    description: "Forwards a message to another channel, as Discord's own Forward does: shown with a link to where it came from. Works across servers the bot is in.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "send",
    aliases: ["Share message", "Repost message", "Quote message"],
    tags: ["forward", "share", "repost", "cross-post", "starboard"],
  },
  look: {
    icon: "send",
  },
  fields: {
    message: { type: "discord.message", pin: "only", required: true, description: "The message to forward." },
    channel: { type: "discord.channel", pin: "only", required: true, description: "Where to forward it." },
  },
  outputs: {
    forwarded: { type: "discord.message", description: "The forwarded message." },
  },
  async run({ fields: f }) {
    if (!f.message?.forward) throw new Error("Connect a message");
    if (!f.channel) throw new Error("Connect a channel");
    return { forwarded: await f.message.forward(f.channel) };
  },
};
