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

module.exports = {
  id: "discord.lyrics",
  kind: "action",
  info: {
    name: "Lyrics",
    description: "Finds a song's lyrics (from lrclib.net): of the track playing on a server, or of a song you name.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "voice",
    aliases: ["Song lyrics", "Song text"],
    tags: ["music", "lyrics", "song", "text"],
  },
  look: {
    icon: "text",
  },
  requires: {
    intents: ["Guilds", "GuildVoiceStates"],
    dependencies: MUSIC,
  },
  fields: {
    server: { type: "discord.guild?", pin: "only", description: "The server: its current track's lyrics." },
    song: { type: "text", default: "", description: "Or a song to look up, such as Alan Walker Faded." },
  },
  outputs: {
    found: { type: "boolean", description: "Whether lyrics were found." },
    lyrics: { type: "text", description: "The lyrics (Discord messages fit 2000 characters; Split Text helps)." },
    title: { type: "text", description: "The song's name." },
    artist: { type: "text", description: "Its artist." },
    synced: { type: "text", description: "Time-stamped lyrics (LRC), when there are any." },
  },
  async run({ fields: f, ctx }) {
    const music = ctx.discord.music;
    let q = String(f.song || "").trim();
    if (!q) {
      const queue = f.server ? await music.queue(f.server) : null;
      const t = queue?.currentTrack;
      if (!t) throw new Error("Name a song, or connect a server where something plays");
      q = `${t.author ?? ""} ${t.cleanTitle || t.title}`.trim();
    }
    const player = await music.get();
    const results = await player.lyrics.search({ q });
    const hit = results.find((r) => r.plainLyrics) ?? results[0];
    if (!hit) return { found: false, lyrics: "", title: "", artist: "", synced: "" };
    return { found: Boolean(hit.plainLyrics), lyrics: hit.plainLyrics ?? "", title: hit.trackName ?? hit.name ?? "", artist: hit.artistName ?? "", synced: hit.syncedLyrics ?? "" };
  },
};
