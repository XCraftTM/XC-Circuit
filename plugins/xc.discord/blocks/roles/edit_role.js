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
const KEEP = { keep: "Keep as is", on: "On", off: "Off" };
const tri = (v) => (v === "on" ? true : v === "off" ? false : undefined);

module.exports = {
  id: "discord.edit_role",
  name: "Edit Role",
  description: "Changes a role's name, color, permissions and more. Empty fields stay as they are.",
  author: "XC Team",
  version: "1.0.0",
  category: "roles",
  icon: "tag",
  kind: "action",
  tags: ["rename", "color", "permissions"],
  fields: {
    role: { type: "discord.role", pin: "only", required: true, description: "The role to change." },
    name: { type: "text", default: "", description: "New name. Empty keeps it." },
    color: { type: "text", default: "", description: "New color such as #FF5CA8. Empty keeps it." },
    hoist: { type: "select", pin: false, default: "keep", choices: KEEP, label: "Show separately", description: "List its members in their own group." },
    mentionable: { type: "select", pin: false, default: "keep", choices: KEEP, description: "Anyone can @mention it." },
    permissions: { type: "text", default: "", description: "Replaces all its permissions (comma separated). Empty keeps them." },
    position: { type: "number", default: 0, min: 0, step: 1, advanced: true, description: "Its place in the role list (0 keeps it). Higher is above." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  outputs: {
    role: { type: "discord.role", description: "The changed role." },
  },
  async run({ fields: f }) {
    const edit = { reason: f.reason || undefined };
    if (f.name) edit.name = String(f.name).slice(0, 100);
    if (f.color) edit.color = f.color;
    if (tri(f.hoist) !== undefined) edit.hoist = tri(f.hoist);
    if (tri(f.mentionable) !== undefined) edit.mentionable = tri(f.mentionable);
    if (String(f.permissions || "").trim()) edit.permissions = permissionList(f.permissions);
    if (f.position > 0) edit.position = Math.floor(f.position);
    return { role: await f.role.edit(edit) };
  },
};
