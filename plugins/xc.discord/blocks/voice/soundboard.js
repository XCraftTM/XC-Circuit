const ACTIONS = { play: "Play a sound", list: "List the server's sounds", create: "Add a sound", delete: "Delete a sound" };

/** A server sound by id or name. */
async function findSound(server, which) {
  const key = String(which || "").trim();
  if (!key) throw new Error("Say which sound (its name or id)");
  const all = await server.soundboardSounds.fetch();
  const sound = all.get(key) ?? all.find((s) => s.name?.toLowerCase() === key.toLowerCase());
  if (!sound) throw new Error(`No sound named ${key}`);
  return sound;
}

module.exports = {
  id: "discord.soundboard",
  kind: "action",
  info: {
    name: "Soundboard",
    description: "Plays a soundboard sound in a voice channel (the bot must be in it, for example through Play Music), lists a server's sounds, or adds and deletes them. Adding needs Manage Expressions and an MP3 or OGG up to 5 seconds and 512 KB.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "voice",
    aliases: ["Play sound", "Sound effect", "Add sound"],
    tags: ["soundboard", "sound", "voice", "effect", "meme"],
  },
  look: {
    icon: "bolt",
  },
  requires: {
    permissions: {
      action: {
        play: ["Connect", "Speak", "UseSoundboard"],
        create: ["ManageGuildExpressions"],
        delete: ["ManageGuildExpressions"],
      },
    },
    intents: ["Guilds", "GuildVoiceStates", "GuildExpressions"],
  },
  fields: {
    action: { type: "select", pin: false, default: "play", choices: ACTIONS, description: "What to do." },
    server: { type: "discord.guild", pin: "only", required: true, description: "The server whose sounds to use." },
    channel: { type: "discord.channel?", pin: "only", show: { action: ["play"] }, label: "Voice channel", description: "Where to play it." },
    sound: { type: "text", default: "", show: { action: ["play", "delete"] }, description: "The sound's name or id." },
    name: { type: "text", default: "", show: { action: ["create"] }, description: "The new sound's name (2 to 32 characters)." },
    file: { type: "text", default: "", show: { action: ["create"] }, label: "File URL", description: "The sound file: a URL or a file path." },
    emoji: { type: "text", default: "", show: { action: ["create"] }, description: "An emoji shown with it (optional)." },
    volume: { type: "number", default: 100, min: 0, max: 100, show: { action: ["create"] }, description: "Its volume in percent." },
  },
  outputs: {
    sound: { type: "object", description: "The played, added or found sound." },
    list: { type: "list<object>", description: "For lists: each name, id, emoji and volume." },
  },
  async run({ fields: f }) {
    const s = f.server;
    if (!s?.soundboardSounds) throw new Error("Connect a server");
    const none = { sound: null, list: [] };
    if (f.action === "list") {
      const all = await s.soundboardSounds.fetch();
      return { ...none, list: [...all.values()].map((x) => ({ name: x.name, id: x.soundId, emoji: x.emoji?.toString() ?? "", volume: Math.round((x.volume ?? 1) * 100) })) };
    }
    if (f.action === "create") {
      const emoji = String(f.emoji || "").trim();
      const custom = /<a?:\w+:(\d+)>/.exec(emoji);
      const sound = await s.soundboardSounds.create({
        file: f.file,
        name: String(f.name).slice(0, 32),
        volume: Math.max(0, Math.min(100, Number(f.volume) || 100)) / 100,
        ...(custom ? { emojiId: custom[1] } : emoji ? { emojiName: emoji } : {}),
      });
      return { ...none, sound };
    }
    const sound = await findSound(s, f.sound);
    if (f.action === "delete") {
      await s.soundboardSounds.delete(sound);
      return none;
    }
    if (!f.channel?.sendSoundboardSound) throw new Error("Connect a voice channel");
    await f.channel.sendSoundboardSound(sound);
    return { ...none, sound };
  },
};
