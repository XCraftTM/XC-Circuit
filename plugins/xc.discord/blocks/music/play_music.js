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
const WHERE = { end: "At the end of the queue", next: "Up next", now: "Right now (skips the current track)" };
const SOURCES = { auto: "Find it anywhere", youtube: "YouTube", spotify: "Spotify", soundcloud: "SoundCloud", deezer: "Deezer", appleMusic: "Apple Music" };

module.exports = {
  id: "discord.play_music",
  kind: "action",
  info: {
    name: "Play Music",
    description: "Joins a voice channel and plays a song, playlist or album: a link (YouTube, Spotify, SoundCloud, a file …) or words to search for. Adds to the queue when something is already playing.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "voice",
    aliases: ["Play song", "Play audio", "Add to queue", "Join and play"],
    tags: ["music", "audio", "song", "youtube", "spotify", "soundcloud", "queue", "voice", "discord-player"],
  },
  look: {
    icon: "bolt",
  },
  requires: {
    permissions: ["ViewChannel", "Connect", "Speak"],
    intents: ["Guilds", "GuildVoiceStates"],
    dependencies: MUSIC,
  },
  fields: {
    query: { type: "text", default: "", placeholder: "Never Gonna Give You Up", label: "Song", description: "A link, or words to search for." },
    member: { type: "discord.member?", pin: "only", description: "Plays in this member's voice channel (they asked for it)." },
    channel: { type: "discord.channel?", pin: "only", label: "Voice channel", description: "Or this voice channel." },
    announce: { type: "discord.channel?", pin: "only", label: "Text channel", description: "Where Music Event blocks can post (Now playing …). Optional." },
    where: { type: "select", pin: false, default: "end", choices: WHERE, description: "Where it goes in the queue." },
    source: { type: "select", pin: false, default: "auto", choices: SOURCES, advanced: true, description: "Where to search when words are given." },
  },
  outputs: {
    track: { type: "object", description: "The (first) track: title, author, url, duration, thumbnail, source." },
    title: { type: "text", description: "Its title." },
    url: { type: "text", description: "Its link." },
    duration: { type: "text", description: "Its length, such as 3:32." },
    thumbnail: { type: "text", description: "Its picture." },
    added: { type: "number", description: "How many tracks were added (more for playlists)." },
    playlist: { type: "text", description: "The playlist's or album's name, if it was one." },
    position: { type: "number", description: "Its place in the queue: 0 = playing now." },
  },
  async run({ fields: f, ctx }) {
    const music = ctx.discord.music;
    const query = String(f.query || "").trim();
    if (!query) throw new Error("Say what to play");
    const voice = f.channel ?? f.member?.voice?.channel ?? null;
    if (!voice?.isVoiceBased?.()) throw new Error(f.member ? "The member isn't in a voice channel" : "Connect a voice channel, or a member who is in one");
    const player = await music.get();
    const requestedBy = f.member?.user ?? undefined;
    const result = await player.search(query, { requestedBy, searchEngine: music.engine(f.source) });
    if (!result.hasTracks()) throw new Error(`Nothing found for ${query}`);
    const tracks = result.playlist ? result.tracks : [result.tracks[0]];

    const queue = player.nodes.create(voice.guild, music.nodeOptions({ channel: f.announce ?? null }));
    if (f.announce) queue.setMetadata({ ...(queue.metadata || {}), channel: f.announce });
    if (!queue.connection) await queue.connect(voice);
    const playing = queue.isPlaying();
    let position;
    if (f.where === "end" || !playing) {
      position = playing ? queue.tracks.size + 1 : 0;
      queue.addTrack(result.playlist ?? tracks);
    } else {
      // Up next: in front of the queue, in order.
      [...tracks].reverse().forEach((t) => queue.insertTrack(t, 0));
      position = 1;
      if (f.where === "now") {
        queue.node.skip();
        position = 0;
      }
    }
    if (!queue.isPlaying()) await queue.node.play();
    const info = music.info(tracks[0]);
    return { track: info, title: info.title, url: info.url, duration: info.duration, thumbnail: info.thumbnail, added: tracks.length, playlist: result.playlist?.title ?? "", position };
  },
};
