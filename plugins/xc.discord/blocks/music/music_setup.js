// npm packages every music block needs: discord-player with its voice, opus and
// ffmpeg parts, Discord's voice encryption (DAVE), and the music sources.
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
  id: "discord.music_setup",
  kind: "event",
  info: {
    name: "Music Player Ready",
    description: "Sets up music: which sources can be played (YouTube, Spotify, SoundCloud …) and how queues behave. Runs once the player is ready. Without this block, music uses YouTube, Spotify and SoundCloud with the defaults. Logins and cookies go in Bot › Settings › Music.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "voice",
    aliases: ["Music setup", "Audio player", "Music sources"],
    tags: ["music", "audio", "player", "youtube", "spotify", "soundcloud", "discord-player"],
  },
  look: {
    icon: "settings",
  },
  requires: {
    intents: ["Guilds", "GuildVoiceStates"],
    dependencies: MUSIC,
  },
  fields: {
    youtube: { type: "boolean", pin: false, default: true, label: "YouTube", description: "Search and play YouTube videos and playlists (also what Spotify links play through)." },
    spotify: { type: "boolean", pin: false, default: true, label: "Spotify", description: "Spotify links and search. Spotify itself can't stream, so tracks play from YouTube or SoundCloud." },
    soundcloud: { type: "boolean", pin: false, default: true, label: "SoundCloud", description: "SoundCloud links and search." },
    deezer: { type: "boolean", pin: false, default: false, label: "Deezer", description: "Deezer links and search. Needs the ARL cookie in Bot › Settings › Music." },
    appleMusic: { type: "boolean", pin: false, default: false, label: "Apple Music", description: "Apple Music links (they play from another source)." },
    tts: { type: "boolean", pin: false, default: false, label: "Text to speech", description: "Play a text read aloud: tts:Hello everyone" },
    ttsLanguage: { type: "text", pin: false, default: "en", label: "Speech language", show: { tts: [true] }, description: "Language code for text to speech, such as en or de." },
    volume: { type: "number", pin: false, default: 80, min: 0, max: 200, label: "Volume", advanced: true, description: "Starting volume of a new queue, in percent." },
    leaveOnEmpty: { type: "boolean", pin: false, default: true, label: "Leave when alone", advanced: true, description: "Leave the voice channel when nobody else is in it." },
    leaveOnEmptySeconds: { type: "number", pin: false, default: 60, min: 0, label: "…after seconds", advanced: true, show: { leaveOnEmpty: [true] }, description: "How long to wait first." },
    leaveOnEnd: { type: "boolean", pin: false, default: true, label: "Leave when done", advanced: true, description: "Leave when the queue has played out." },
    leaveOnEndSeconds: { type: "number", pin: false, default: 60, min: 0, label: "…after seconds", advanced: true, show: { leaveOnEnd: [true] }, description: "How long to wait first." },
    selfDeaf: { type: "boolean", pin: false, default: true, label: "Deafen the bot", advanced: true, description: "The bot doesn't listen in voice (saves bandwidth)." },
  },
  outputs: {
    sources: { type: "list<text>", description: "The sources that loaded." },
  },
  listen(ctx, emit) {
    const f = ctx.fields;
    ctx.discord.music.configure({
      youtube: f.youtube,
      spotify: f.spotify,
      soundcloud: f.soundcloud,
      deezer: f.deezer,
      appleMusic: f.appleMusic,
      tts: f.tts,
      ttsLanguage: f.ttsLanguage,
      volume: f.volume,
      leaveOnEmpty: f.leaveOnEmpty,
      leaveOnEmptySeconds: f.leaveOnEmptySeconds,
      leaveOnEnd: f.leaveOnEnd,
      leaveOnEndSeconds: f.leaveOnEndSeconds,
      selfDeaf: f.selfDeaf,
    });
    ctx.discord.music.get().then(
      (player) => emit({ sources: player.xcSources || [] }),
      (e) => ctx.log.error(`The music player couldn't start: ${e.message}`),
    );
  },
};
