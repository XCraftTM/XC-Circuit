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
const SOURCES = { auto: "Find it anywhere", youtube: "YouTube", spotify: "Spotify", soundcloud: "SoundCloud", deezer: "Deezer", appleMusic: "Apple Music" };

module.exports = {
  id: "discord.search_music",
  kind: "action",
  info: {
    name: "Search Music",
    description: "Searches for songs without playing them: for a pick-a-song menu, or the choices of an Autocomplete (each result's url plays it with Play Music).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "voice",
    aliases: ["Find song", "Song search", "Music autocomplete"],
    tags: ["music", "audio", "search", "song", "autocomplete", "youtube", "spotify", "soundcloud"],
  },
  look: {
    icon: "zoom-in",
  },
  requires: {
    intents: ["Guilds", "GuildVoiceStates"],
    dependencies: MUSIC,
  },
  fields: {
    query: { type: "text", default: "", label: "Search for", description: "Words or a link." },
    source: { type: "select", pin: false, default: "auto", choices: SOURCES, description: "Where to search." },
    limit: { type: "number", default: 10, min: 1, max: 25, description: "How many results at most (25 fit in a menu or autocomplete)." },
  },
  outputs: {
    results: { type: "list<object>", description: "Each result: title, author, url, duration, thumbnail, source." },
    choices: { type: "list<object>", description: "The results as Autocomplete choices: name (title · duration) and value (url)." },
    first: { type: "object", description: "The best result." },
    count: { type: "number", description: "How many were found." },
  },
  async run({ fields: f, ctx }) {
    const music = ctx.discord.music;
    const query = String(f.query || "").trim();
    if (!query) return { results: [], choices: [], first: null, count: 0 };
    const player = await music.get();
    const result = await player.search(query, { searchEngine: music.engine(f.source) });
    const results = result.tracks.slice(0, Math.max(1, Math.min(25, Number(f.limit) || 10))).map(music.info);
    return {
      results,
      choices: results.map((t) => ({ name: `${t.title} · ${t.duration}`.slice(0, 100), value: t.url.slice(0, 100) })),
      first: results[0] ?? null,
      count: results.length,
    };
  },
};
