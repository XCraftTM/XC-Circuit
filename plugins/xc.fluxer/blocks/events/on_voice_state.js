const WHEN = { join: "Joins a voice channel", leave: "Leaves voice", move: "Moves to another channel", any: "Any voice change" };

module.exports = {
  id: "fluxer.on_voice_state",
  kind: "event",
  info: {
    name: "Voice Channel Joined or Left",
    description: "Runs when someone joins, leaves or switches voice channels. Also for mute and deafen changes with Any voice change.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "calls",
    aliases: ["On voice", "Joined voice", "Left voice", "Voice state"],
  },
  look: {
    icon: "volume",
  },
  fields: {
    when: { type: "select", pin: false, default: "join", choices: WHEN, label: "When", description: "Which change." },
  },
  outputs: {
    userId: { type: "text", label: "User id", description: "Who." },
    member: { type: "fluxer.member?", description: "Who, as a server member, when known." },
    channelId: { type: "text", label: "Channel id", description: "The voice channel they are in now (empty when they left)." },
    beforeChannelId: { type: "text", label: "Channel before", description: "The voice channel they were in (empty when they joined)." },
    serverId: { type: "text", label: "Server id", description: "The server." },
    muted: { type: "boolean", description: "Whether they are muted." },
    deafened: { type: "boolean", description: "Whether they are deafened." },
  },
  listen(ctx, emit) {
    // Fluxer sends only the new state: the channel before is remembered here.
    const where = new Map();
    ctx.fluxer.client.on("voiceStateUpdate", (d) => {
      const userId = String(d?.user_id ?? d?.userId ?? "");
      const serverId = String(d?.guild_id ?? d?.guildId ?? "");
      const key = `${serverId}:${userId}`;
      const after = String(d?.channel_id ?? d?.channelId ?? "");
      const before = where.get(key) ?? "";
      if (after) where.set(key, after);
      else where.delete(key);
      const kind = !before && after ? "join" : before && !after ? "leave" : before !== after ? "move" : "other";
      if (ctx.fields.when !== "any" && ctx.fields.when !== kind) return;
      emit({
        userId,
        member: d?.member ?? null,
        channelId: after,
        beforeChannelId: before,
        serverId,
        muted: Boolean(d?.self_mute ?? d?.mute ?? d?.selfMute),
        deafened: Boolean(d?.self_deaf ?? d?.deaf ?? d?.selfDeaf),
      });
    });
  },
};
