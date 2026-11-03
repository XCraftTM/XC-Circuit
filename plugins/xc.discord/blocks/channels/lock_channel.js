const ACTIONS = { lock: "Lock (nobody can write)", unlock: "Unlock" };

module.exports = {
  id: "discord.lock_channel",
  kind: "action",
  info: {
    name: "Lock Channel",
    description: "Stops (or lets again) a role, @everyone by default, from writing in a channel.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["Lockdown", "Unlock", "Kanal sperren"],
    tags: ["lockdown", "unlock", "mute channel"],
  },
  look: {
    icon: "shield",
  },
  fields: {
    channel: { type: "discord.channel", pin: "only", required: true, description: "The channel." },
    action: { type: "select", pin: false, default: "lock", choices: ACTIONS, description: "Lock or unlock." },
    role: { type: "discord.role?", pin: "only", description: "Who is locked out. Empty = @everyone." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  outputs: {
    channel: { type: "discord.channel", description: "The same channel." },
  },
  async run({ fields: f }) {
    const id = f.role?.id ?? f.channel.guild.roles.everyone.id;
    // Unlocking goes back to the role's default (null), not to an explicit allow.
    const value = f.action === "lock" ? false : null;
    await f.channel.permissionOverwrites.edit(id, { SendMessages: value, SendMessagesInThreads: value, AddReactions: value }, { reason: f.reason || undefined });
    return { channel: f.channel };
  },
};
