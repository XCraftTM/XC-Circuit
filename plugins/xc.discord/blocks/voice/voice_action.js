const ACTIONS = { move: "Move to a channel", disconnect: "Disconnect", mute: "Server mute", unmute: "Unmute", deafen: "Server deafen", undeafen: "Undeafen" };

module.exports = {
  id: "discord.voice_action",
  name: "Voice Member",
  description: "Moves a member to another voice channel, disconnects, mutes or deafens them. The bot needs Move, Mute or Deafen Members.",
  author: "XC Team",
  version: "1.0.0",
  category: "voice",
  icon: "bolt",
  kind: "action",
  tags: ["move", "disconnect", "kick from voice", "mute", "deafen", "vc"],
  fields: {
    member: { type: "discord.member", pin: "only", required: true, description: "Who (must be in a voice channel)." },
    action: { type: "select", pin: false, default: "move", choices: ACTIONS, description: "What to do." },
    channel: { type: "discord.channel", pin: "only", required: true, show: { action: ["move"] }, description: "The voice channel to move them to." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  outputs: {
    member: { type: "discord.member", description: "The same member." },
  },
  async run({ fields: f }) {
    const v = f.member.voice;
    const reason = f.reason || undefined;
    if (!v.channelId && f.action !== "unmute" && f.action !== "undeafen") throw new Error(`${f.member.displayName} isn't in a voice channel`);
    const act = {
      move: () => v.setChannel(f.channel, reason),
      disconnect: () => v.disconnect(reason),
      mute: () => v.setMute(true, reason),
      unmute: () => v.setMute(false, reason),
      deafen: () => v.setDeaf(true, reason),
      undeafen: () => v.setDeaf(false, reason),
    }[f.action];
    await act();
    return { member: f.member };
  },
};
