const CHANGES = { any: "Any change", join: "Joins a voice channel", leave: "Leaves voice", move: "Moves to another channel", mute: "Mutes or unmutes", deaf: "Deafens or undeafens", stream: "Starts or stops streaming", video: "Turns the camera on or off" };

module.exports = {
  id: "discord.on_voice_state",
  kind: "event",
  info: {
    name: "Voice Changed",
    description: "Runs when someone joins, leaves or moves between voice channels, mutes, deafens or streams. For temporary voice channels and voice logs.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "voice",
    aliases: ["Joined voice", "Left voice"],
    tags: ["voice", "join", "leave", "temp channel", "vc"],
  },
  look: {
    icon: "bolt",
  },
  requires: {
    intents: ["Guilds", "GuildVoiceStates"],
  },
  fields: {
    change: { type: "select", pin: false, default: "any", choices: CHANGES, description: "Which change runs it." },
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Skip bots (such as music bots)." },
  },
  outputs: {
    member: { type: "discord.member", description: "Who changed." },
    before: { type: "discord.channel?", description: "The voice channel they were in (empty when joining)." },
    after: { type: "discord.channel?", description: "The voice channel they are in now (empty when leaving)." },
    muted: { type: "boolean", description: "Whether they are muted now (by themselves or the server)." },
    deafened: { type: "boolean", description: "Whether they are deafened now." },
    streaming: { type: "boolean", description: "Whether they are streaming now." },
    server: { type: "discord.guild", description: "The server." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("voiceStateUpdate", (old, now) => {
      const member = now.member ?? old.member;
      if (!member || (ctx.fields.ignoreBots && member.user.bot)) return;
      const changed = {
        join: !old.channelId && now.channelId,
        leave: old.channelId && !now.channelId,
        move: old.channelId && now.channelId && old.channelId !== now.channelId,
        mute: old.mute !== now.mute,
        deaf: old.deaf !== now.deaf,
        stream: Boolean(old.streaming) !== Boolean(now.streaming),
        video: Boolean(old.selfVideo) !== Boolean(now.selfVideo),
      };
      const which = ctx.fields.change;
      if (which === "any" ? !Object.values(changed).some(Boolean) : !changed[which]) return;
      emit({ member, before: old.channel ?? null, after: now.channel ?? null, muted: Boolean(now.mute), deafened: Boolean(now.deaf), streaming: Boolean(now.streaming), server: now.guild });
    });
  },
};
