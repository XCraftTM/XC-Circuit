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

const WHO = { member: "A member", bot: "The bot" };

module.exports = {
  id: "discord.check_permission",
  kind: "value",
  info: {
    name: "Check Permission",
    description: "Whether a member (or the bot) has a permission, in the server or in one channel.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    tags: ["can", "allowed", "admin", "mod only"],
  },
  look: {
    icon: "shield",
  },
  fields: {
    who: { type: "select", pin: false, default: "member", choices: WHO, description: "Whose permission to check." },
    member: { type: "discord.member", pin: "only", required: true, show: { who: ["member"] }, description: "The member." },
    server: { type: "discord.guild?", pin: "only", show: { who: ["bot"] }, description: "The server (not needed when a channel is connected)." },
    permission: { type: "select", pin: false, default: "ManageMessages", choices: PERMISSIONS, description: "The permission." },
    channel: { type: "discord.channel?", pin: "only", description: "Check in this channel (its overrides count). Empty = the whole server." },
  },
  outputs: {
    result: { type: "boolean", description: "True when the permission is there. Administrators have every permission." },
  },
  run({ fields: f }) {
    const { PermissionFlagsBits } = require("discord.js");
    const member = f.who === "bot" ? (f.channel?.guild ?? f.server)?.members.me : f.member;
    if (!member) throw new Error("Connect a server or channel to check the bot's permissions");
    const perms = f.channel ? f.channel.permissionsFor(member) : member.permissions;
    return { result: Boolean(perms && perms.has(PermissionFlagsBits[f.permission])) };
  },
};
