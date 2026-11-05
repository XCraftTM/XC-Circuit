const ACTIONS = { create: "Create a webhook", list: "List webhooks", delete: "Delete a webhook" };
const WHERE = { channel: "In one channel", server: "In the whole server" };

module.exports = {
  id: "fluxer.webhooks",
  kind: "action",
  info: {
    name: "Webhooks",
    description: "Creates, lists or deletes webhooks: a way to post in a channel with any name and picture. Post with Send with Webhook.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["Create webhook", "Delete webhook", "Webhook list"],
  },
  look: {
    icon: "send",
  },
  requires: {
    permissions: ["ManageWebhooks"],
  },
  fields: {
    action: { type: "select", pin: false, default: "create", choices: ACTIONS, description: "What to do." },
    where: { type: "select", pin: false, default: "channel", choices: WHERE, show: { action: ["list"] }, description: "Which webhooks to list." },
    channel: { type: "fluxer.channel", pin: "only", show: { action: ["create", "list"] }, description: "The channel." },
    server: { type: "fluxer.server", pin: "only", show: { action: ["list"] }, description: "For the whole server: the server." },
    webhook: { type: "fluxer.webhook", pin: "only", show: { action: ["delete"] }, description: "The webhook to delete." },
    name: { type: "text", default: "Announcements", show: { action: ["create"] }, description: "The name it posts with." },
    avatar: { type: "text", default: "", show: { action: ["create"] }, description: "A link to the picture it posts with. Empty = the default one." },
  },
  outputs: {
    webhook: { type: "fluxer.webhook?", description: "The new webhook." },
    key: { type: "text", description: "The new webhook's key (id/token), for Send with Webhook. Keep it secret: anyone with it can post." },
    webhooks: { type: "list<fluxer.webhook>", description: "For lists: the webhooks." },
  },
  async run({ fields: f, ctx }) {
    if (f.action === "delete") {
      if (!f.webhook) throw new Error("Connect the webhook");
      await f.webhook.delete();
      return { webhook: null, key: "", webhooks: [] };
    }
    if (f.action === "list") {
      if (f.where === "server") {
        const server = await ctx.fluxer.server(f.server ?? f.channel);
        return { webhook: null, key: "", webhooks: await server.fetchWebhooks() };
      }
      const channel = await ctx.fluxer.channel(f.channel);
      return { webhook: null, key: "", webhooks: await channel.fetchWebhooks() };
    }
    const channel = await ctx.fluxer.channel(f.channel);
    const name = String(f.name ?? "").trim() || "Webhook";
    const webhook = await channel.createWebhook({ name: name.slice(0, 80), avatar: (await ctx.fluxer.imageData(f.avatar)) ?? null });
    return { webhook, key: webhook.token ? `${webhook.id}/${webhook.token}` : "", webhooks: [] };
  },
};
