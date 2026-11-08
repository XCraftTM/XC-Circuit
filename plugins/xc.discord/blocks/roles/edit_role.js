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

/** Permission flag names from a list of them (the field's choices), or text such as "Send Messages, manage roles" (older projects). */
function permissionList(given) {
  // Items with commas in them too (a text wired in becomes a list of one).
  const items = (Array.isArray(given) ? given : [given]).flatMap((s) => String(s ?? "").split(/[,\n]/));
  const wanted = items.map((s) => s.trim()).filter(Boolean);
  const squash = (s) => s.toLowerCase().replace(/[^a-z]/g, "");
  return wanted.map((w) => {
    const hit = Object.entries(PERMISSIONS).find(([flag, label]) => squash(flag) === squash(w) || squash(label) === squash(w));
    if (!hit) throw new Error(`Unknown permission "${w}". Known: ${Object.values(PERMISSIONS).join(", ")}`);
    return hit[0];
  });
}
const KEEP = { keep: "Keep as is", on: "On", off: "Off" };
const tri = (v) => (v === "on" ? true : v === "off" ? false : undefined);

/** A colour as a number: hex such as #FF5CA8 or FF5CA8 (#F5A, F5A), or a number. Empty: undefined. */
function hexColor(v) {
  if (typeof v === "number" && Number.isFinite(v)) return v & 0xffffff;
  const s = String(v ?? "").trim().replace(/^#/, "");
  if (!s) return undefined;
  if (/^[0-9a-f]{6}$/i.test(s)) return parseInt(s, 16);
  if (/^[0-9a-f]{3}$/i.test(s)) return parseInt(s.replace(/./g, "$&$&"), 16);
  throw new Error(`"${String(v).trim()}" isn't a hex color: use one such as #FF5CA8 or FF5CA8`);
}

module.exports = {
  id: "discord.edit_role",
  kind: "action",
  info: {
    name: "Edit Role",
    description: "Changes a role's name, color, permissions and more. Empty fields stay as they are.",
    author: "XC Team",
    version: "1.1.2",
  },
  search: {
    category: "roles",
    aliases: ["Role color", "Rename role"],
    tags: ["rename", "color", "permissions"],
  },
  look: {
    icon: "tag",
  },
  requires: {
    permissions: ["ManageRoles"],
  },
  fields: {
    role: { type: "discord.role", pin: "only", required: true, description: "The role to change." },
    name: { type: "text", default: "", description: "New name. Empty keeps it." },
    color: { type: "text", default: "", description: "New color in hex, such as #FF5CA8 or FF5CA8. Empty keeps it." },
    hoist: { type: "select", pin: false, default: "keep", choices: KEEP, label: "Show separately", description: "List its members in their own group." },
    mentionable: { type: "select", pin: false, default: "keep", choices: KEEP, description: "Anyone can @mention it." },
    permissions: { type: "multiselect", default: [], choices: PERMISSIONS, description: "Replaces all its permissions. None picked keeps them." },
    position: { type: "number", default: 0, min: 0, step: 1, advanced: true, description: "Its place in the role list (0 keeps it). Higher is above." },
    emoji: { type: "text", default: "", advanced: true, label: "Icon emoji", description: "An emoji shown next to members' names. Empty keeps it." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  outputs: {
    role: { type: "discord.role", description: "The changed role." },
  },
  async run({ fields: f }) {
    const edit = { reason: f.reason || undefined };
    if (f.name) edit.name = String(f.name).slice(0, 100);
    if (hexColor(f.color) !== undefined) edit.color = hexColor(f.color);
    if (tri(f.hoist) !== undefined) edit.hoist = tri(f.hoist);
    if (tri(f.mentionable) !== undefined) edit.mentionable = tri(f.mentionable);
    if (permissionList(f.permissions).length) edit.permissions = permissionList(f.permissions);
    if (f.position > 0) edit.position = Math.floor(f.position);
    if (f.emoji) edit.unicodeEmoji = f.emoji;
    return { role: await f.role.edit(edit) };
  },
};
