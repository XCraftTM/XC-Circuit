const TYPES = { text: "Text", voice: "Voice", category: "Category", announcement: "Announcement", forum: "Forum", stage: "Stage" };

module.exports = {
  id: "discord.create_channel",
  name: "Create Channel",
  description: "Makes a new channel or category. The bot needs Manage Channels.",
  author: "XC Team",
  version: "1.0.0",
  category: "channels",
  icon: "hash",
  kind: "action",
  tags: ["new channel", "ticket", "category", "voice"],
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "Where to create it." },
    type: { type: "select", pin: false, default: "text", choices: TYPES, description: "What kind of channel." },
    name: { type: "text", required: true, default: "new-channel", description: "The channel's name." },
    category: { type: "discord.channel?", pin: "only", description: "The category to put it in. Empty = none." },
    topic: { type: "text", default: "", show: { type: ["text", "announcement", "forum"] }, description: "The description at the top." },
    private: { type: "boolean", default: false, description: "Hidden from @everyone (give access with Set Channel Permissions)." },
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
    if (f.private) options.permissionOverwrites = [{ id: f.server.roles.everyone.id, deny: [PermissionFlagsBits.ViewChannel] }, { id: f.server.members.me.id, allow: [PermissionFlagsBits.ViewChannel] }];
    return { channel: await f.server.channels.create(options) };
  },
};
