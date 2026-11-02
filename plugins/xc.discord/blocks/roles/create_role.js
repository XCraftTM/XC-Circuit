// Permission names, as typed: "Send Messages, Manage Roles" (case and spaces don't matter).
const PERMISSIONS = {
  Administrator: "Administrator",
  ViewChannel: "View Channel",
  SendMessages: "Send Messages",
  SendMessagesInThreads: "Send Messages in Threads",
  ReadMessageHistory: "Read Message History",
  AddReactions: "Add Reactions",
  EmbedLinks: "Embed Links",
  AttachFiles: "Attach Files",
  MentionEveryone: "Mention Everyone",
  UseExternalEmojis: "Use External Emojis",
  UseApplicationCommands: "Use Application Commands",
  ManageMessages: "Manage Messages",
  ManageThreads: "Manage Threads",
  CreatePublicThreads: "Create Public Threads",
  CreatePrivateThreads: "Create Private Threads",
  ManageChannels: "Manage Channels",
  ManageRoles: "Manage Roles",
  ManageGuild: "Manage Server",
  ManageNicknames: "Manage Nicknames",
  ManageWebhooks: "Manage Webhooks",
  ManageGuildExpressions: "Manage Expressions",
  ManageEvents: "Manage Events",
  KickMembers: "Kick Members",
  BanMembers: "Ban Members",
  ModerateMembers: "Time Out Members",
  ViewAuditLog: "View Audit Log",
  ChangeNickname: "Change Nickname",
  CreateInstantInvite: "Create Invite",
  Connect: "Connect",
  Speak: "Speak",
  Stream: "Video",
  MuteMembers: "Mute Members",
  DeafenMembers: "Deafen Members",
  MoveMembers: "Move Members",
  PrioritySpeaker: "Priority Speaker",
};

/** Permission flag names from text such as "Send Messages, manage roles". */
function permissionList(text) {
  const wanted = String(text || "").split(/[,\n]/).map((s) => s.trim()).filter(Boolean);
  const squash = (s) => s.toLowerCase().replace(/[^a-z]/g, "");
  return wanted.map((w) => {
    const hit = Object.entries(PERMISSIONS).find(([flag, label]) => squash(flag) === squash(w) || squash(label) === squash(w));
    if (!hit) throw new Error(`Unknown permission "${w}". Known: ${Object.values(PERMISSIONS).join(", ")}`);
    return hit[0];
  });
}

module.exports = {
  id: "discord.create_role",
  name: "Create Role",
  description: "Makes a new role in a server. The bot needs Manage Roles.",
  author: "XC Team",
  version: "1.0.0",
  category: "roles",
  icon: "tag",
  kind: "action",
  tags: ["new role", "rank"],
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "Where to create it." },
    name: { type: "text", required: true, default: "New role", description: "The role's name." },
    color: { type: "color", default: "#99AAB5", description: "The name color in the member list." },
    hoist: { type: "boolean", default: false, label: "Show separately", description: "List its members in their own group." },
    mentionable: { type: "boolean", default: false, description: "Anyone can @mention it." },
    permissions: { type: "text", default: "", description: "Comma separated, such as Send Messages, Manage Messages. Empty = none." },
    emoji: { type: "text", default: "", advanced: true, label: "Icon emoji", description: "An emoji shown next to members' names (needs boost level 2)." },
    position: { type: "number", default: 0, min: 0, step: 1, advanced: true, description: "Its place in the role list (0 = just above @everyone)." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  outputs: {
    role: { type: "discord.role", description: "The new role." },
  },
  async run({ fields: f }) {
    const role = await f.server.roles.create({
      name: String(f.name || "New role").slice(0, 100),
      color: f.color || undefined,
      hoist: Boolean(f.hoist),
      mentionable: Boolean(f.mentionable),
      permissions: permissionList(f.permissions),
      unicodeEmoji: f.emoji || undefined,
      position: f.position > 0 ? Math.floor(f.position) : undefined,
      reason: f.reason || undefined,
    });
    return { role };
  },
};
