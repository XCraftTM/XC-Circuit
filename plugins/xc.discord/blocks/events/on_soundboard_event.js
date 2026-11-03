const KINDS = {
  played: "A sound or effect is used in voice",
  created: "A sound is added",
  updated: "A sound is changed",
  deleted: "A sound is deleted",
};

module.exports = {
  id: "discord.on_soundboard_event",
  kind: "event",
  info: {
    name: "Soundboard Used",
    description: "Runs when someone plays a soundboard sound or sends an emoji effect in a voice channel, or when a server's soundboard sounds change.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_members",
    aliases: ["Sound played", "Voice effect", "Soundboard sound added"],
    tags: ["soundboard", "sound", "voice", "effect"],
  },
  look: {
    icon: "bolt",
  },
  requires: {
    intents: ["Guilds", "GuildVoiceStates", "GuildExpressions"],
  },
  fields: {
    kind: { type: "select", pin: false, default: "played", label: "When", choices: KINDS, description: "Which event runs it." },
  },
  outputs: {
    user: { type: "discord.user?", description: "For used sounds: who played it." },
    channel: { type: "discord.channel?", description: "For used sounds: the voice channel." },
    name: { type: "text", description: "The sound's name (when known)." },
    soundId: { type: "text", label: "Sound id", description: "The sound's id, empty for an emoji effect." },
    emoji: { type: "text", description: "The sound's or effect's emoji." },
    sound: { type: "object", description: "The sound, when known." },
    server: { type: "discord.guild?", description: "The server." },
  },
  listen(ctx, emit) {
    const c = ctx.discord.client;
    const kind = ctx.fields.kind;
    const fromSound = (sound) => ({ user: sound.user ?? null, channel: null, name: sound.name ?? "", soundId: sound.soundId ?? "", emoji: sound.emoji?.toString() ?? "", sound, server: sound.guild ?? null });
    if (kind === "played") {
      c.on("voiceChannelEffectSend", async (effect) => {
        const sound = effect.soundId ? (effect.soundboardSound ?? effect.guild?.soundboardSounds?.cache.get(effect.soundId) ?? null) : null;
        const user = await c.users.fetch(effect.userId).catch(() => null);
        emit({ user, channel: effect.channel ?? null, name: sound?.name ?? "", soundId: effect.soundId ? String(effect.soundId) : "", emoji: effect.emoji?.toString() ?? "", sound, server: effect.guild ?? null });
      });
    } else if (kind === "updated") {
      c.on("guildSoundboardSoundUpdate", (old, now) => emit(fromSound(now)));
    } else {
      c.on(kind === "deleted" ? "guildSoundboardSoundDelete" : "guildSoundboardSoundCreate", (sound) => emit(fromSound(sound)));
    }
  },
};
