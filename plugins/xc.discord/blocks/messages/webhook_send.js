const TARGETS = { channel: "A channel (the bot makes a webhook)", url: "A webhook URL" };

module.exports = {
  id: "discord.webhook_send",
  kind: "action",
  info: {
    name: "Send via Webhook",
    description: "Sends a message under any name and avatar, through a webhook.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "send",
    aliases: ["Webhook"],
    tags: ["impersonate", "custom name", "avatar", "announce"],
  },
  look: {
    icon: "send",
  },
  fields: {
    target: { type: "select", pin: false, default: "channel", choices: TARGETS, description: "Where it goes." },
    channel: { type: "discord.channel", pin: "only", required: true, show: { target: ["channel"] }, description: "The channel. The bot needs Manage Webhooks." },
    url: { type: "text", default: "", show: { target: ["url"] }, description: "A webhook URL from a channel's settings." },
    name: { type: "text", default: "Announcements", description: "The name shown on the message." },
    avatar: { type: "text", default: "", label: "Avatar URL", description: "The picture shown. Empty = the webhook's own." },
    text: { type: "textarea", default: "", description: "The message text." },
    embeds: { type: "list<discord.embed>", pin: "only", multi: true, description: "Embeds to attach." },
  },
  outputs: {
    message: { type: "discord.message?", description: "The sent message (when sent through a channel)." },
  },
  async run({ fields: f }) {
    const payload = { content: f.text || undefined, username: f.name || undefined, avatarURL: f.avatar || undefined, embeds: (f.embeds || []).flat().filter(Boolean) };
    if (f.target === "url") {
      const { WebhookClient } = require("discord.js");
      await new WebhookClient({ url: String(f.url) }).send(payload);
      return { message: null };
    }
    const channel = f.channel.isThread?.() ? f.channel.parent : f.channel;
    const hooks = await channel.fetchWebhooks();
    const hook = hooks.find((h) => h.owner?.id === channel.client.user.id) ?? (await channel.createWebhook({ name: "XC Circuit" }));
    if (f.channel.isThread?.()) payload.threadId = f.channel.id;
    return { message: await hook.send(payload) };
  },
};
