// Fluxer's permissions, by their flag names (as @fluxerjs/util names them).
const PERMISSIONS = {
  ViewChannel: "View Channel",
  SendMessages: "Send Messages",
  ReadMessageHistory: "Read Message History",
  AddReactions: "Add Reactions",
  EmbedLinks: "Embed Links",
  AttachFiles: "Attach Files",
  MentionEveryone: "Mention Everyone",
  UseExternalEmojis: "Use External Emojis",
  UseExternalStickers: "Use External Stickers",
  SendTtsMessages: "Send Text-to-Speech Messages",
  PinMessages: "Pin Messages",
  ManageMessages: "Manage Messages",
  BypassSlowmode: "Bypass Slowmode",
  ManageChannels: "Manage Channel",
  ManageRoles: "Manage Permissions",
  ManageWebhooks: "Manage Webhooks",
  CreateInstantInvite: "Create Invite",
  ViewChannelMembers: "View Channel Members",
  Connect: "Connect",
  Speak: "Speak",
  Stream: "Video",
  UseVad: "Use Voice Activity",
  MuteMembers: "Mute Members",
  DeafenMembers: "Deafen Members",
  MoveMembers: "Move Members",
  PrioritySpeaker: "Priority Speaker",
  UpdateRtcRegion: "Change Voice Region",
};

const FOR = { role: "A role", member: "A member", everyone: "Everyone" };
const MODES = { change: "Allow or deny", reset: "Reset (back to the role's)" };

module.exports = {
  id: "fluxer.set_permissions",
  kind: "action",
  info: {
    name: "Set Channel Permissions",
    description: "Allows or denies permissions in one channel for a role, a member or everyone, or resets them. For private channels and locking.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "channels",
    aliases: ["Channel permissions", "Lock channel", "Hide channel", "Private channel", "Permission overwrite"],
  },
  look: {
    icon: "key",
  },
  requires: {
    permissions: ["ManageChannels", "ManageRoles"],
  },
  fields: {
    channel: { type: "fluxer.channel", pin: "only", required: true, description: "The channel." },
    for: { type: "select", pin: false, default: "role", choices: FOR, description: "Whose permissions." },
    who: { type: "any", pin: true, show: { for: ["role", "member"] }, description: "The role or member (or their id)." },
    mode: { type: "select", pin: false, default: "change", choices: MODES, description: "What to do." },
    allow: { type: "multiselect", default: [], choices: PERMISSIONS, show: { mode: ["change"] }, description: "Permissions to allow." },
    deny: { type: "multiselect", default: [], choices: PERMISSIONS, show: { mode: ["change"] }, description: "Permissions to deny. Deny Send Messages to lock a channel." },
  },
  outputs: {
    channel: { type: "fluxer.channel", description: "The same channel." },
  },
  async run({ fields: f, ctx }) {
    const channel = await ctx.fluxer.channel(f.channel);
    if (!channel.permissionOverwrites) throw new Error("This channel has no permissions of its own");
    let targetId;
    let type;
    if (f.for === "everyone") {
      targetId = String(channel.guildId ?? channel.guild?.id ?? "");
      type = 0;
    } else {
      targetId = ctx.fluxer.id(f.who, f.for === "role" ? "role" : "user");
      type = f.for === "role" ? 0 : 1;
    }
    if (!targetId) throw new Error(f.for === "role" ? "Connect the role, or type its id" : "Connect the member, or type their id");
    if (f.mode === "reset") {
      await channel.permissionOverwrites.delete(targetId);
      return { channel };
    }
    // A list of flag names (the field's choices), or text with commas (older projects).
    const list = (s) => (Array.isArray(s) ? s : String(s ?? "").split(/[,\s]+/)).map((p) => String(p).trim()).filter(Boolean);
    const allow = list(f.allow);
    const deny = list(f.deny);
    if (!allow.length && !deny.length) throw new Error("Name permissions to allow or deny");
    await channel.permissionOverwrites.edit(targetId, { type, allow: allow.length ? allow : undefined, deny: deny.length ? deny : undefined });
    return { channel };
  },
};
