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

const WHO = { everyone: "@everyone", target: "A role or member (connect it)" };
const MODES = { change: "Allow and deny", reset: "Back to the defaults (remove the override)" };

module.exports = {
  id: "discord.set_permissions",
  kind: "action",
  info: {
    name: "Set Channel Permissions",
    description: "Allows or denies permissions on one channel for a role or a member, such as giving a ticket's creator access.",
    author: "XC Team",
    version: "1.1.1",
  },
  search: {
    category: "roles",
    aliases: ["Permission overwrite", "Channel access"],
    tags: ["overwrite", "access", "private", "ticket"],
  },
  look: {
    icon: "shield",
  },
  requires: {
    permissions: ["ManageChannels", "ManageRoles"],
  },
  fields: {
    channel: { type: "discord.channel", pin: "only", required: true, description: "The channel." },
    who: { type: "select", pin: false, default: "target", choices: WHO, description: "Whose permissions change." },
    target: { type: "any", pin: "only", required: true, show: { who: ["target"] }, description: "A role, member or user (or an id)." },
    mode: { type: "select", pin: false, default: "change", choices: MODES, description: "Change permissions, or remove the override." },
    allow: { type: "multiselect", default: ["ViewChannel", "SendMessages"], choices: PERMISSIONS, show: { mode: ["change"] }, description: "Permissions to allow." },
    deny: { type: "multiselect", default: [], choices: PERMISSIONS, show: { mode: ["change"] }, description: "Permissions to deny." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  outputs: {
    channel: { type: "discord.channel", description: "The same channel." },
  },
  async run({ fields: f }) {
    const id = f.who === "everyone" ? f.channel.guild.roles.everyone.id : f.target?.id ?? String(f.target);
    if (f.mode === "reset") {
      await f.channel.permissionOverwrites.delete(id, f.reason || undefined);
      return { channel: f.channel };
    }
    const changes = {};
    for (const p of permissionList(f.allow)) changes[p] = true;
    for (const p of permissionList(f.deny)) changes[p] = false;
    await f.channel.permissionOverwrites.edit(id, changes, { reason: f.reason || undefined });
    return { channel: f.channel };
  },
};
