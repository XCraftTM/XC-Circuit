const ACTIONS = { create: "Create a webhook", list: "List webhooks", edit: "Change a webhook", delete: "Delete a webhook" };
const WHERE = { channel: "In one channel", server: "In the whole server" };

/** A webhook from a connected object or a webhook URL. */
async function findWebhook(client, f) {
  if (f.webhook && typeof f.webhook === "object") return f.webhook;
  const m = /webhooks\/(\d+)\/([\w-]+)/.exec(String(f.url || f.webhook || ""));
  if (!m) throw new Error("Connect a webhook or give its URL");
  return client.fetchWebhook(m[1], m[2]);
}

const info = (w) => ({ id: w.id, name: w.name, url: w.url ?? "", channelId: w.channelId ?? "", avatar: w.avatarURL?.() ?? "", creator: w.owner?.username ?? "" });

module.exports = {
  id: "discord.webhooks",
  kind: "action",
  info: {
    name: "Webhooks",
    description: "Creates, lists, changes (name, picture, channel) or deletes webhooks. Send via Webhook posts with them. The bot needs Manage Webhooks.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["Create webhook", "Delete webhook", "Manage webhooks"],
    tags: ["webhook", "integration", "url"],
  },
  look: {
    icon: "plug",
  },
  fields: {
    action: { type: "select", pin: false, default: "create", choices: ACTIONS, description: "What to do." },
    where: { type: "select", pin: false, default: "channel", choices: WHERE, show: { action: ["list"] }, description: "Which webhooks to list." },
    channel: { type: "discord.channel?", pin: "only", show: { action: ["create", "list", "edit"] }, description: "The channel (for edit: where to move it, optional)." },
    server: { type: "discord.guild?", pin: "only", show: { action: ["list"], where: ["server"] }, description: "The server, for listing all its webhooks." },
    webhook: { type: "object", pin: "only", show: { action: ["edit", "delete"] }, description: "A webhook from this block or a list. Or give its URL below." },
    url: { type: "text", template: false, default: "", show: { action: ["edit", "delete"] }, label: "Webhook URL", description: "Instead of connecting one." },
    name: { type: "text", default: "", show: { action: ["create", "edit"] }, description: "Its name. Empty when changing = keep." },
    avatar: { type: "text", default: "", show: { action: ["create", "edit"] }, label: "Avatar URL", description: "Its picture. Empty when changing = keep." },
  },
  outputs: {
    webhook: { type: "object", description: "The created or changed webhook." },
    url: { type: "text", description: "Its URL (keep it secret: anyone with it can post)." },
    list: { type: "list<object>", description: "For lists: each id, name, url, channelId, avatar and creator." },
    webhooks: { type: "list<object>", description: "For lists: the webhook objects." },
  },
  async run({ fields: f, ctx }) {
    const client = ctx.discord.client;
    const none = { webhook: null, url: "", list: [], webhooks: [] };
    if (f.action === "create") {
      if (!f.channel?.createWebhook) throw new Error("Connect a text channel");
      const w = await f.channel.createWebhook({ name: String(f.name || "Webhook").slice(0, 80), avatar: f.avatar || undefined });
      return { ...none, webhook: w, url: w.url };
    }
    if (f.action === "list") {
      const source = f.where === "server" ? f.server : f.channel;
      if (!source?.fetchWebhooks) throw new Error(f.where === "server" ? "Connect a server" : "Connect a text channel");
      const all = [...(await source.fetchWebhooks()).values()];
      return { ...none, list: all.map(info), webhooks: all };
    }
    const w = await findWebhook(client, f);
    if (f.action === "delete") {
      await w.delete();
      return none;
    }
    const edit = {};
    if (f.name) edit.name = String(f.name).slice(0, 80);
    if (f.avatar) edit.avatar = f.avatar;
    if (f.channel) edit.channel = f.channel;
    const changed = await w.edit(edit);
    return { ...none, webhook: changed, url: changed.url ?? "" };
  },
};
