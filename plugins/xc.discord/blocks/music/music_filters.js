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
const ACTIONS = { toggle: "Turn a filter on or off", on: "Turn a filter on", off: "Turn a filter off", clear: "Turn every filter off", list: "List the filters that are on" };
// discord-player's ffmpeg filters, with names people know.
const FILTERS = {
  bassboost_low: "Bass boost (low)",
  bassboost: "Bass boost",
  bassboost_high: "Bass boost (high)",
  nightcore: "Nightcore",
  vaporwave: "Vaporwave",
  "8D": "8D audio",
  lofi: "Lo-fi",
  karaoke: "Karaoke (less vocals)",
  treble: "Treble",
  reverse: "Reverse",
  tremolo: "Tremolo",
  vibrato: "Vibrato",
  phaser: "Phaser",
  flanger: "Flanger",
  chorus: "Chorus",
  surrounding: "Surround",
  pulsator: "Pulsator",
  subboost: "Sub boost",
  mono: "Mono",
  normalizer: "Normalize loudness",
  compressor: "Compressor",
  softlimiter: "Soft limiter",
  fadein: "Fade in",
  dim: "Dim",
  earrape: "Earrape (very loud)",
};

module.exports = {
  id: "discord.music_filters",
  kind: "action",
  info: {
    name: "Music Filters",
    description: "Sound effects for the music on a server: bass boost, nightcore, 8D, lo-fi, karaoke and more. Changing them restarts the track at the same place.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "voice",
    aliases: ["Audio filter", "Bass boost", "Nightcore", "8D audio", "Equalizer"],
    tags: ["music", "audio", "filter", "effect", "bass", "nightcore", "8d", "lofi", "karaoke"],
  },
  look: {
    icon: "settings",
  },
  requires: {
    intents: ["Guilds", "GuildVoiceStates"],
    dependencies: MUSIC,
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    action: { type: "select", pin: false, default: "toggle", choices: ACTIONS, description: "What to do." },
    filter: { type: "select", pin: false, default: "bassboost", choices: FILTERS, show: { action: ["toggle", "on", "off"] }, description: "Which filter." },
  },
  outputs: {
    enabled: { type: "list<text>", description: "The filters that are on now." },
    on: { type: "boolean", description: "Whether the chosen filter is on now." },
  },
  async run({ fields: f, ctx }) {
    const queue = await ctx.discord.music.needQueue(f.server);
    const ff = queue.filters.ffmpeg;
    const has = () => ff.getFiltersEnabled().includes(f.filter);
    if (f.action === "clear") await ff.setFilters(false);
    else if (f.action === "toggle") await ff.toggle(f.filter);
    else if (f.action !== "list" && has() !== (f.action === "on")) await ff.toggle(f.filter);
    return { enabled: ff.getFiltersEnabled().map((k) => FILTERS[k] ?? k), on: has() };
  },
};
