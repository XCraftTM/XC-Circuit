const TYPES = { text: "Text", voice: "Voice", category: "Category", announcement: "Announcement", forum: "Forum", stage: "Stage" };

module.exports = {
  id: "discord.create_channel",
  kind: "action",
  info: {
    name: "Create Channel",
    description: "Makes a new channel or category. The bot needs Manage Channels.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    tags: ["new channel", "ticket", "category", "voice"],
  },
  look: {
    icon: "hash",
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "Where to create it." },
    type: { type: "select", pin: false, default: "text", choices: TYPES, description: "What kind of channel." },
    name: { type: "text", required: true, default: "new-channel", description: "The channel's name." },
    category: { type: "discord.channel?", pin: "only", description: "The category to put it in. Empty = none." },
    topic: { type: "text", default: "", show: { type: ["text", "announcement", "forum"] }, description: "The description at the top." },
    private: { type: "boolean", default: false, description: "Hidden from @everyone (give access with Set Channel Permissions)." },
    nsfw: { type: "boolean", default: false, advanced: true, label: "Age-restricted", show: { type: ["text", "announcement", "forum", "voice"] }, description: "Marks it NSFW." },
    slowmode: { type: "number", default: 0, min: 0, max: 21600, step: 1, advanced: true, label: "Slowmode (s)", show: { type: ["text", "forum", "voice"] }, description: "Seconds between messages per person." },
    bitrate: { type: "number", default: 64, min: 8, max: 384, step: 1, advanced: true, label: "Bitrate (kbps)", show: { type: ["voice", "stage"] }, description: "Audio quality. Above 96 needs boosts." },
    userLimit: { type: "number", default: 0, min: 0, max: 99, step: 1, advanced: true, label: "User limit", show: { type: ["voice"] }, description: "How many can join. 0 = no limit." },
    position: { type: "number", default: -1, min: -1, step: 1, advanced: true, description: "Its place in the list. -1 = at the end." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  outputs: {
    channel: { type: "discord.channel", description: "The new channel." },
  },
  async run({ fields: f }) {
    const { ChannelType, PermissionFlagsBits } = require("discord.js");
    const type = { text: ChannelType.GuildText, voice: ChannelType.GuildVoice, category: ChannelType.GuildCategory, announcement: ChannelType.GuildAnnouncement, forum: ChannelType.GuildForum, stage: ChannelType.GuildStageVoice }[f.type];
    const options = { name: String(f.name || "new-channel").slice(0, 100), type, reason: f.reason || undefined };
    if (f.category && f.type !== "category") options.parent = f.category.id ?? f.category;
    if (f.topic) options.topic = String(f.topic).slice(0, 1024);
    if (f.nsfw) options.nsfw = true;
    if (f.slowmode > 0) options.rateLimitPerUser = Math.floor(f.slowmode);
    if (f.type === "voice" || f.type === "stage") options.bitrate = Math.max(8, Math.floor(f.bitrate || 64)) * 1000;
    if (f.type === "voice" && f.userLimit > 0) options.userLimit = Math.floor(f.userLimit);
    if (f.position >= 0) options.position = Math.floor(f.position);
    if (f.private) options.permissionOverwrites = [{ id: f.server.roles.everyone.id, deny: [PermissionFlagsBits.ViewChannel] }, { id: f.server.members.me.id, allow: [PermissionFlagsBits.ViewChannel] }];
    return { channel: await f.server.channels.create(options) };
  },
};
