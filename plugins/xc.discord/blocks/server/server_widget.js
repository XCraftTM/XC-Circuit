const ACTIONS = { info: "Get the widget (who is online)", settings: "Get its settings", set: "Change its settings" };
const STYLES = { shield: "Shield", banner1: "Banner 1", banner2: "Banner 2", banner3: "Banner 3", banner4: "Banner 4" };

module.exports = {
  id: "discord.server_widget",
  kind: "action",
  info: {
    name: "Server Widget",
    description: "Reads a server's widget (online count, members shown, invite link, an image for websites), or turns it on and off and sets its invite channel. Changing it needs Manage Server.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Guild widget", "Online count", "Widget image"],
    tags: ["widget", "online", "website", "embed", "invite"],
  },
  look: {
    icon: "layers",
  },
  fields: {
    action: { type: "select", pin: false, default: "info", choices: ACTIONS, description: "What to do." },
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    enabled: { type: "boolean", pin: false, default: true, show: { action: ["set"] }, description: "Whether the widget is on." },
    channel: { type: "discord.channel?", pin: "only", show: { action: ["set"] }, description: "The channel its invite leads to. Empty = none." },
    style: { type: "select", pin: false, default: "banner2", choices: STYLES, show: { action: ["info"] }, label: "Image style", description: "The look of the widget image." },
  },
  outputs: {
    enabled: { type: "boolean", description: "Whether the widget is on." },
    online: { type: "number", description: "How many members are online." },
    members: { type: "list<object>", description: "The members it shows (up to 100): username, status, avatar." },
    invite: { type: "text", description: "Its invite link, when it has a channel." },
    image: { type: "text", description: "An image of it, for websites and embeds." },
    channel: { type: "discord.channel?", description: "Its invite channel." },
  },
  async run({ fields: f }) {
    const s = f.server;
    if (!s?.fetchWidgetSettings) throw new Error("Connect a server");
    const image = `https://discord.com/api/guilds/${s.id}/widget.png?style=${f.style || "banner2"}`;
    const base = { enabled: false, online: 0, members: [], invite: "", image, channel: null };
    if (f.action === "set") {
      await s.setWidgetSettings({ enabled: Boolean(f.enabled), channel: f.channel ?? null });
      return { ...base, enabled: Boolean(f.enabled), channel: f.channel ?? null };
    }
    const settings = await s.fetchWidgetSettings();
    base.enabled = Boolean(settings.enabled);
    base.channel = settings.channel ?? null;
    if (f.action === "settings" || !settings.enabled) return base;
    const w = await s.fetchWidget();
    return {
      ...base,
      online: w.presenceCount ?? 0,
      members: [...w.members.values()].map((m) => ({ username: m.username, status: m.status, avatar: m.avatarURL ?? "" })),
      invite: w.instantInvite ?? "",
      image: w.imageURL?.(f.style) ?? image,
    };
  },
};
