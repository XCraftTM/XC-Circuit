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
// Block choice → discord-player event.
const KINDS = {
  playerStart: "A track starts playing",
  playerFinish: "A track finishes",
  audioTrackAdd: "A track is added to the queue",
  audioTracksAdd: "A playlist is added to the queue",
  playerSkip: "A track is skipped (couldn't play)",
  emptyQueue: "The queue has played out",
  emptyChannel: "Everyone left the voice channel",
  disconnect: "The bot left the voice channel",
  playerError: "A track fails to play",
};

module.exports = {
  id: "discord.music_event",
  kind: "event",
  info: {
    name: "Music Event",
    description: "Runs when the music on a server changes: a track starts or ends, tracks are added, the queue runs out, the bot is left alone or disconnected, or a track fails. Text channel is the one Play Music was given.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "voice",
    aliases: ["Now playing", "Track started", "Song ended", "Queue empty", "Music error"],
    tags: ["music", "audio", "track", "queue", "player", "event"],
  },
  look: {
    icon: "bolt",
  },
  requires: {
    intents: ["Guilds", "GuildVoiceStates"],
    dependencies: MUSIC,
  },
  fields: {
    kind: { type: "select", pin: false, default: "playerStart", label: "When", choices: KINDS, description: "Which event runs it." },
  },
  outputs: {
    server: { type: "discord.guild", description: "The server." },
    track: { type: "object", description: "The track (title, author, url, duration, thumbnail, source, requestedBy), when there is one." },
    title: { type: "text", description: "The track's title." },
    url: { type: "text", description: "Its link." },
    count: { type: "number", description: "For playlists: how many tracks were added." },
    requestedBy: { type: "discord.user?", label: "Requested by", description: "Who asked for the track." },
    channel: { type: "discord.channel?", label: "Text channel", description: "The text channel Play Music was given, to post in." },
    voiceChannel: { type: "discord.channel?", label: "Voice channel", description: "Where the bot plays." },
    error: { type: "text", description: "For failures: what went wrong." },
  },
  listen(ctx, emit) {
    const kind = ctx.fields.kind;
    ctx.discord.music.get().then(
      (player) =>
        player.events.on(kind, (queue, a, b) => {
          const tracks = kind === "audioTracksAdd" ? a : null;
          const track = kind === "playerError" ? b : tracks ? tracks[0] : a && typeof a === "object" && "title" in a ? a : null;
          const info = ctx.discord.music.info(track);
          emit({
            server: queue.guild,
            track: info,
            title: info?.title ?? "",
            url: info?.url ?? "",
            count: tracks ? tracks.length : track ? 1 : 0,
            requestedBy: track?.requestedBy ?? null,
            channel: queue.metadata?.channel ?? null,
            voiceChannel: queue.channel ?? null,
            error: kind === "playerError" ? String(a?.message ?? a) : kind === "playerSkip" ? String(b ?? "") : "",
          });
        }),
      (e) => ctx.log.error(`The music player couldn't start: ${e.message}`),
    );
  },
};
