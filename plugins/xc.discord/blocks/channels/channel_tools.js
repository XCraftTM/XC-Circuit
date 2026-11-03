const ACTIONS = {
  list: "List a server's channels",
  clone: "Copy a channel",
  sync: "Sync permissions with its category",
  follow: "Follow an announcement channel",
};
const TYPES = { all: "All", text: "Text", voice: "Voice", category: "Categories", announcement: "Announcement", forum: "Forum", stage: "Stage", thread: "Threads" };
const TYPE_NUMBERS = { text: [0], voice: [2], category: [4], announcement: [5], forum: [15, 16], stage: [13], thread: [10, 11, 12] };

module.exports = {
  id: "discord.channel_tools",
  kind: "action",
  info: {
    name: "Channel Tools",
    description: "Lists a server's channels (by kind), copies a channel with its permissions, syncs a channel's permissions with its category, or follows an announcement channel so its posts appear in another channel. Changes need Manage Channels (following: Manage Webhooks).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["List channels", "Clone channel", "Copy channel", "Sync permissions", "Follow announcements"],
    tags: ["channel", "clone", "copy", "list", "sync", "permissions", "announcement", "follow"],
  },
  look: {
    icon: "hash",
  },
  fields: {
    action: { type: "select", pin: false, default: "list", choices: ACTIONS, description: "What to do." },
    server: { type: "discord.guild?", pin: "only", show: { action: ["list"] }, description: "The server." },
    type: { type: "select", pin: false, default: "all", choices: TYPES, show: { action: ["list"] }, label: "Kind", description: "Which channels to list." },
    channel: { type: "discord.channel?", pin: "only", show: { action: ["clone", "sync", "follow"] }, description: "The channel (for follow: the announcement channel)." },
    name: { type: "text", default: "", show: { action: ["clone"] }, description: "The copy's name. Empty = the same name." },
    target: { type: "discord.channel?", pin: "only", show: { action: ["follow"] }, label: "Post into", description: "The text channel its posts appear in." },
  },
  outputs: {
    channel: { type: "discord.channel?", description: "The copied channel." },
    channels: { type: "list<discord.channel>", description: "For lists: the channels, in their order." },
    list: { type: "list<object>", description: "For lists: each id, name, kind number and category id." },
  },
  async run({ fields: f }) {
    const none = { channel: null, channels: [], list: [] };
    if (f.action === "list") {
      if (!f.server?.channels) throw new Error("Connect a server");
      const all = [...(await f.server.channels.fetch()).values()].filter(Boolean);
      const want = TYPE_NUMBERS[f.type];
      const channels = all.filter((c) => !want || want.includes(c.type)).sort((a, b) => (a.rawPosition ?? 0) - (b.rawPosition ?? 0));
      return { ...none, channels, list: channels.map((c) => ({ id: c.id, name: c.name, type: c.type, categoryId: c.parentId ?? "" })) };
    }
    const c = f.channel;
    if (!c) throw new Error("Connect a channel");
    if (f.action === "clone") return { ...none, channel: await c.clone(f.name ? { name: String(f.name).slice(0, 100) } : {}) };
    if (f.action === "sync") {
      if (!c.parentId) throw new Error("The channel isn't in a category");
      return { ...none, channel: await c.lockPermissions() };
    }
    if (!c.addFollower) throw new Error("Connect an announcement channel");
    if (!f.target) throw new Error("Connect the channel to post into");
    await c.addFollower(f.target);
    return { ...none, channel: f.target };
  },
};
