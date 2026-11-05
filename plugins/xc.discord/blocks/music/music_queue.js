// npm packages every music block needs (the list in Music Player Ready).
const MUSIC = {
  "discord-player": "^7.2.0",
  "@discord-player/extractor": "^7.2.0",
  mediaplex: "^1.0.0",
  "@snazzah/davey": "^0.2.0",
  "ffmpeg-static": "^5.3.0",
  "discord-player-youtubei": "^3.1.0",
  "simple-ytdl-core": "^1.2.2",
  "bgutils-js": "^4.0.3",
  googlevideo: "^4.1.1",
  undici: "^7.30.1",
  "discord-player-spotify": "^1.1.9",
  "discord-player-soundcloud": "^1.0.7",
  "discord-player-deezer": "^2.6.0",
  "discord-player-applemusic": "^1.0.3",
  "discord-player-tts": "^1.0.9",
};
const REPEAT = ["off", "track", "queue", "autoplay"];

module.exports = {
  id: "discord.music_queue",
  kind: "value",
  pure: false,
  info: {
    name: "Music Queue",
    description: "What plays on a server: the current track with its progress bar, what comes next, the volume and repeat mode. For Now Playing and Queue commands.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "voice",
    aliases: ["Now playing", "Current song", "Queue list", "Progress bar"],
    tags: ["music", "audio", "queue", "now playing", "progress", "track"],
  },
  look: {
    icon: "repeat",
  },
  requires: {
    intents: ["Guilds", "GuildVoiceStates"],
    dependencies: MUSIC,
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    limit: { type: "number", default: 10, min: 1, max: 100, description: "How many upcoming tracks to list." },
  },
  outputs: {
    playing: { type: "boolean", description: "Whether something plays (or is paused)." },
    paused: { type: "boolean", description: "Whether it is paused." },
    current: { type: "object", description: "The current track: title, author, url, duration, thumbnail, source, requestedBy." },
    title: { type: "text", description: "The current track's title." },
    url: { type: "text", description: "Its link." },
    progress: { type: "text", description: "Where it is, such as 1:23 / 3:45." },
    bar: { type: "text", label: "Progress bar", description: "A text progress bar, such as ▬▬▬🔘▬▬▬▬." },
    upcoming: { type: "list<object>", description: "The next tracks, up to the limit." },
    list: { type: "text", description: "The next tracks as numbered lines, ready for a message." },
    size: { type: "number", description: "How many tracks are waiting." },
    length: { type: "text", description: "How long the waiting tracks are together." },
    volume: { type: "number", description: "The volume in percent." },
    repeat: { type: "text", description: "off, track, queue or autoplay." },
    voiceChannel: { type: "discord.voice_channel?", label: "Voice channel", description: "Where the bot plays." },
  },
  async run({ fields: f, ctx }) {
    const music = ctx.discord.music;
    const queue = await music.queue(f.server);
    const empty = { playing: false, paused: false, current: null, title: "", url: "", progress: "", bar: "", upcoming: [], list: "", size: 0, length: "0:00", volume: 0, repeat: "off", voiceChannel: null };
    if (!queue) return empty;
    const current = queue.currentTrack;
    const stamp = current ? queue.node.getTimestamp() : null;
    const upcoming = queue.tracks.toArray().slice(0, Math.max(1, Number(f.limit) || 10));
    return {
      playing: Boolean(current),
      paused: queue.node.isPaused(),
      current: music.info(current),
      title: current?.cleanTitle || current?.title || "",
      url: current?.url ?? "",
      progress: stamp ? `${stamp.current.label} / ${stamp.total.label}` : "",
      bar: current ? (queue.node.createProgressBar() ?? "") : "",
      upcoming: upcoming.map(music.info),
      list: upcoming.map((t, i) => `${i + 1}. ${t.title} (${t.duration})`).join("\n"),
      size: queue.tracks.size,
      length: queue.durationFormatted ?? "",
      volume: queue.node.volume,
      repeat: REPEAT[queue.repeatMode] ?? "off",
      voiceChannel: queue.channel ?? null,
    };
  },
};
