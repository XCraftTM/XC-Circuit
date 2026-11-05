module.exports = {
  id: "fluxer.webhook_send",
  kind: "action",
  info: {
    name: "Send with Webhook",
    description: "Posts a message through a webhook, with any name and picture: for announcements, logs or characters.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "send",
    aliases: ["Webhook message", "Post as", "Send as"],
  },
  look: {
    icon: "send",
  },
  fields: {
    webhook: { type: "fluxer.webhook", pin: "only", description: "A webhook from Webhooks. Or give its key below." },
    key: { type: "text", default: "", label: "Webhook key", description: "When no webhook is connected: its key (id/token, from Webhooks) or its link (…/webhooks/id/token)." },
    text: { type: "textarea", default: "", description: "What to post." },
    username: { type: "text", default: "", label: "Name", description: "The name to post with. Empty = the webhook's." },
    avatar: { type: "text", default: "", label: "Picture link", description: "The picture to post with. Empty = the webhook's." },
    embed: { type: "fluxer.embed", pin: "only", description: "An embed from Build Embed (optional)." },
  },
  outputs: {
    message: { type: "fluxer.message?", description: "The posted message, when Fluxer gives it back." },
  },
  async run({ fields: f, ctx }) {
    let webhook = f.webhook;
    if (!webhook) {
      const m = /(?:webhooks\/|^)(\d+)\/([\w-]+)\/?$/.exec(String(f.key ?? "").trim());
      if (!m) throw new Error("Connect a webhook, or give its key (id/token) or link");
      const { Webhook } = require("@fluxerjs/core");
      webhook = Webhook.fromToken(ctx.fluxer.client, m[1], m[2]);
    }
    const options = {};
    const content = String(f.text ?? "");
    if (content.trim()) options.content = content.slice(0, 2000);
    if (f.embed) options.embeds = [f.embed];
    if (!options.content && !options.embeds) throw new Error("Give some text or an embed");
    if (String(f.username ?? "").trim()) options.username = String(f.username).trim().slice(0, 80);
    if (String(f.avatar ?? "").trim()) options.avatarUrl = String(f.avatar).trim();
    const message = await webhook.send(options, true);
    return { message: message ?? null };
  },
};
