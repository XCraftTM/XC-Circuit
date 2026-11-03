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
const ACTIONS = {
  pause: "Pause",
  resume: "Resume",
  togglePause: "Pause or resume",
  skip: "Skip to the next track",
  back: "Go back to the previous track",
  stop: "Stop and clear the queue",
  leave: "Leave the voice channel",
  shuffle: "Shuffle the queue",
  clear: "Clear the queue (keep the current track)",
  volume: "Set the volume",
  seek: "Jump to a time in the track",
  repeat: "Set repeat",
  jump: "Play a track from the queue now",
  remove: "Remove a track from the queue",
  move: "Move a track in the queue",
  selfMute: "Mute or unmute the bot",
};
const REPEAT = { off: "Off", track: "Repeat the track", queue: "Repeat the queue", autoplay: "Autoplay similar tracks" };
const REPEAT_MODES = { off: 0, track: 1, queue: 2, autoplay: 3 };

module.exports = {
  id: "discord.music_control",
  kind: "action",
  info: {
    name: "Music Control",
    description: "Controls the music on a server: pause, resume, skip, go back, stop, leave, shuffle, volume, seek, repeat, and changing the queue.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "voice",
    aliases: ["Skip song", "Pause music", "Stop music", "Volume", "Loop", "Shuffle queue"],
    tags: ["music", "audio", "skip", "pause", "resume", "stop", "volume", "seek", "repeat", "loop", "shuffle", "queue"],
  },
  look: {
    icon: "settings",
  },
  requires: {
    intents: ["Guilds", "GuildVoiceStates"],
    dependencies: MUSIC,
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server whose music to control." },
    action: { type: "select", pin: false, default: "skip", choices: ACTIONS, description: "What to do." },
    volume: { type: "number", default: 80, min: 0, max: 200, show: { action: ["volume"] }, description: "Volume in percent." },
    seconds: { type: "number", default: 0, min: 0, show: { action: ["seek"] }, label: "Time (seconds)", description: "Where to jump to in the current track." },
    repeat: { type: "select", pin: false, default: "off", choices: REPEAT, show: { action: ["repeat"] }, description: "How to repeat." },
    position: { type: "number", default: 1, min: 1, show: { action: ["jump", "remove", "move"] }, description: "The track's place in the queue: 1 = up next." },
    to: { type: "number", default: 1, min: 1, show: { action: ["move"] }, label: "New place", description: "Where to move it: 1 = up next." },
    mute: { type: "boolean", default: true, show: { action: ["selfMute"] }, description: "Muted or not." },
  },
  outputs: {
    result: { type: "text", description: "What happened, such as Skipped Never Gonna Give You Up." },
    track: { type: "object", description: "The track it was about (skipped, removed, now playing …), if any." },
  },
  async run({ fields: f, ctx }) {
    const music = ctx.discord.music;
    const queue = await music.needQueue(f.server);
    const at = (n) => {
      const t = queue.tracks.at(Math.max(0, Math.floor(Number(n) || 1) - 1));
      if (!t) throw new Error(`The queue has no track ${n} (it has ${queue.tracks.size})`);
      return t;
    };
    const current = queue.currentTrack;
    const done = (result, track = null) => ({ result, track: music.info(track) });
    switch (f.action) {
      case "pause":
        queue.node.pause();
        return done("Paused", current);
      case "resume":
        queue.node.resume();
        return done("Resumed", current);
      case "togglePause": {
        const paused = queue.node.isPaused();
        queue.node.setPaused(!paused);
        return done(paused ? "Resumed" : "Paused", current);
      }
      case "skip":
        queue.node.skip();
        return done(`Skipped ${current?.title ?? "the track"}`, current);
      case "back":
        if (!queue.history.previousTrack) throw new Error("There is no previous track");
        await queue.history.back();
        return done(`Back to ${queue.currentTrack?.title ?? "the previous track"}`, queue.currentTrack);
      case "stop":
        queue.tracks.clear();
        queue.node.stop();
        return done("Stopped");
      case "leave":
        queue.delete();
        return done("Left the voice channel");
      case "shuffle":
        queue.tracks.shuffle();
        return done(`Shuffled ${queue.tracks.size} tracks`);
      case "clear":
        queue.tracks.clear();
        return done("Cleared the queue", current);
      case "volume": {
        const v = Math.max(0, Math.min(200, Number(f.volume) || 0));
        queue.node.setVolume(v);
        return done(`Volume ${v}%`, current);
      }
      case "seek": {
        const ms = Math.max(0, Number(f.seconds) || 0) * 1000;
        if (!(await queue.node.seek(ms))) throw new Error("This track can't jump to a time");
        return done(`Jumped to ${Math.floor(ms / 60000)}:${String(Math.floor(ms / 1000) % 60).padStart(2, "0")}`, current);
      }
      case "repeat":
        queue.setRepeatMode(REPEAT_MODES[f.repeat] ?? 0);
        return done(`Repeat: ${REPEAT[f.repeat] ?? "Off"}`, current);
      case "jump": {
        const t = at(f.position);
        queue.node.skipTo(t);
        return done(`Playing ${t.title}`, t);
      }
      case "remove": {
        const t = at(f.position);
        queue.node.remove(t);
        return done(`Removed ${t.title}`, t);
      }
      case "move": {
        const t = at(f.position);
        queue.node.move(t, Math.max(0, Math.floor(Number(f.to) || 1) - 1));
        return done(`Moved ${t.title} to ${f.to}`, t);
      }
      case "selfMute":
        await queue.setSelfMute(Boolean(f.mute));
        return done(f.mute ? "Muted" : "Unmuted", current);
    }
    throw new Error(`Unknown action ${f.action}`);
  },
};
