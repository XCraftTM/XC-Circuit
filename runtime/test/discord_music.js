// Runs the music blocks against a stand-in discord-player (player, queue,
// tracks) and checks what they do and return. Real playback needs Discord.
// Usage: node runtime/test/discord_music.js
"use strict";

const path = require("path");
const assert = require("assert");
const { EventEmitter } = require("events");
const { ungroupBlock } = require("../circuit-runtime.js");
const { trackInfo } = require("../../plugins/xc.discord/runtime/discord.js");

const blocks = path.join(__dirname, "..", "..", "plugins", "xc.discord", "blocks");
const load = (rel) => ungroupBlock(require(path.join(blocks, rel)));

const track = (title, extra = {}) => ({ title, cleanTitle: title, author: "A", url: `https://x/${title}`, duration: "3:00", durationMS: 180000, thumbnail: "t", ...extra });

/** A queue like discord-player's GuildQueue, recording what it was asked. */
function makeQueue(guild) {
  const items = [];
  const q = {
    guild,
    channel: { id: "voice" },
    metadata: null,
    connection: null,
    current: null,
    paused: false,
    volume: 80,
    repeatMode: 0,
    filters: [],
    log: [],
    tracks: {
      toArray: () => [...items],
      at: (i) => items[i],
      get size() {
        return items.length;
      },
      clear: () => items.splice(0),
      shuffle: () => items.reverse(),
    },
    history: { previousTrack: track("Before"), back: async () => (q.current = track("Before")) },
    get currentTrack() {
      return q.current;
    },
    isPlaying: () => Boolean(q.current),
    setMetadata: (m) => (q.metadata = m),
    connect: async (c) => (q.connection = c),
    addTrack: (t) => items.push(...(Array.isArray(t) ? t : t.tracks ? t.tracks : [t])),
    insertTrack: (t, i) => items.splice(i, 0, t),
    setRepeatMode: (m) => (q.repeatMode = m),
    delete: () => q.log.push("delete"),
    durationFormatted: "6:00",
    node: {
      play: async () => (q.current = items.shift()),
      skip: () => (q.current = items.shift() ?? null),
      pause: () => (q.paused = true),
      resume: () => (q.paused = false),
      isPaused: () => q.paused,
      setPaused: (p) => (q.paused = p),
      stop: () => (q.current = null),
      setVolume: (v) => (q.volume = v),
      get volume() {
        return q.volume;
      },
      seek: async () => true,
      skipTo: (t) => ((q.current = t), items.splice(items.indexOf(t), 1)),
      remove: (t) => items.splice(items.indexOf(t), 1),
      move: (t, to) => (items.splice(items.indexOf(t), 1), items.splice(to, 0, t)),
      getTimestamp: () => ({ current: { label: "1:00" }, total: { label: "3:00" } }),
      createProgressBar: () => "▬🔘▬",
    },
    filters_: null,
  };
  q.filters = {
    ffmpeg: {
      on: [],
      getFiltersEnabled: () => q.filters.ffmpeg.on,
      toggle: async (n) => (q.filters.ffmpeg.on = q.filters.ffmpeg.on.includes(n) ? q.filters.ffmpeg.on.filter((x) => x !== n) : [...q.filters.ffmpeg.on, n]),
      setFilters: async () => (q.filters.ffmpeg.on = []),
    },
  };
  return q;
}

const guild = { id: "g" };
const queues = new Map();
let searched;
const player = {
  xcSources: ["YouTube", "Spotify"],
  events: new EventEmitter(),
  nodes: { create: (g) => queues.get(g) ?? queues.set(g, makeQueue(g)).get(g), get: (g) => queues.get(g) ?? null },
  search: async (query, opts) => {
    searched = opts;
    if (query === "nothing") return { hasTracks: () => false, tracks: [] };
    if (query === "playlist") return { hasTracks: () => true, tracks: [track("P1"), track("P2")], playlist: { title: "Mix", tracks: [track("P1"), track("P2")] } };
    return { hasTracks: () => true, tracks: [track(query), track(`${query} 2`)] };
  },
  lyrics: { search: async ({ q }) => [{ trackName: q, artistName: "B", plainLyrics: "la la", syncedLyrics: "[00:01] la" }] },
};
let configured = {};
const music = {
  configure: (o) => (configured = { ...configured, ...o }),
  get: async () => player,
  nodeOptions: (metadata) => ({ metadata }),
  queue: async (s) => player.nodes.get(s),
  needQueue: async (s) => {
    const q = player.nodes.get(s);
    if (!q) throw new Error("Nothing is playing on this server");
    return q;
  },
  engine: (source) => (source === "youtube" ? "ext:yt" : "auto"),
  info: trackInfo,
};

// What a music search may look for: words and web addresses, files only in
// the bot's files folder. Never a path on its computer.
{
  const { musicQuery } = require("../../plugins/xc.discord/runtime/discord.js");
  const os = require("os");
  const fsx = require("fs");
  const data = fsx.mkdtempSync(path.join(os.tmpdir(), "xc-music-"));
  fsx.mkdirSync(path.join(data, "files"));
  fsx.writeFileSync(path.join(data, "files", "song.mp3"), "");
  music.query = (q) => musicQuery(data, q);
  assert.strictEqual(musicQuery(data, " never gonna give you up "), "never gonna give you up");
  assert.strictEqual(musicQuery(data, "https://youtu.be/dQw4w9WgXcQ"), "https://youtu.be/dQw4w9WgXcQ");
  assert.strictEqual(musicQuery(data, "song.mp3"), path.join(data, "files", "song.mp3"), "a file the bot keeps, by its name");
  for (const q of ["C:\\Windows\\win.ini", "c:/Windows/win.ini", "/etc/passwd", "\\\\server\\share\\a.mp3", "~/music.mp3", "./a.mp3", "../files/song.mp3", "file:///etc/passwd", __filename]) {
    assert.throws(() => musicQuery(data, q), /not files on the bot's computer/, q);
  }
}

async function run(rel, fields) {
  const block = load(rel);
  const defaults = Object.fromEntries(Object.entries(block.fields).map(([k, f]) => [k, f.default ?? null]));
  const out = await block.run({ fields: { ...defaults, ...fields }, ctx: { discord: { music } } });
  for (const key of Object.keys(block.outputs)) assert.ok(key in out, `${block.id}: output ${key}`);
  return out;
}

(async () => {
  // Every music block asks for the same packages, so any one installs them all.
  const deps = load("music/music_setup.js").dependencies;
  assert.ok(deps["discord-player"] && deps["discord-player-youtubei"] && deps["@snazzah/davey"]);
  for (const f of ["play_music", "music_control", "music_queue", "music_filters", "music_event", "search_music", "lyrics"]) {
    assert.deepStrictEqual(load(`music/${f}.js`).dependencies, deps, `${f}: same packages as Music Player Ready`);
  }

  // Setup passes its choices on and runs once the player is ready.
  const setup = load("music/music_setup.js");
  const emitted = [];
  setup.listen({ fields: { youtube: true, spotify: false, volume: 50 }, discord: { music }, log: console }, (o) => emitted.push(o));
  await new Promise((r) => setImmediate(r));
  assert.deepStrictEqual([configured.spotify, configured.volume, emitted[0].sources], [false, 50, ["YouTube", "Spotify"]]);

  // Play: joins the member's channel, plays, then queues.
  const member = { user: { id: "u" }, voice: { channel: { guild, isVoiceBased: () => true } } };
  let o = await run("music/play_music.js", { query: "Song", member, source: "youtube" });
  const q = queues.get(guild);
  assert.deepStrictEqual([o.title, o.position, q.currentTrack.title, searched.searchEngine, searched.requestedBy.id], ["Song", 0, "Song", "ext:yt", "u"]);
  o = await run("music/play_music.js", { query: "playlist", member });
  assert.deepStrictEqual([o.added, o.playlist, o.position, q.tracks.size], [2, "Mix", 1, 2]);
  o = await run("music/play_music.js", { query: "Next", member, where: "next" });
  assert.deepStrictEqual([o.position, q.tracks.toArray()[0].title], [1, "Next"]);
  await assert.rejects(run("music/play_music.js", { query: "nothing", member }), /Nothing found/);
  await assert.rejects(run("music/play_music.js", { query: "x", member: { voice: {} } }), /isn't in a voice channel/);

  // Queue info.
  o = await run("music/music_queue.js", { server: guild, limit: 2 });
  assert.deepStrictEqual([o.title, o.progress, o.bar, o.size, o.list.split("\n")[0]], ["Song", "1:00 / 3:00", "▬🔘▬", 3, "1. Next (3:00)"]);
  o = await run("music/music_queue.js", { server: { id: "other" } });
  assert.strictEqual(o.playing, false, "no queue: nothing plays");

  // Control.
  o = await run("music/music_control.js", { server: guild, action: "togglePause" });
  assert.deepStrictEqual([o.result, q.paused], ["Paused", true]);
  o = await run("music/music_control.js", { server: guild, action: "skip" });
  assert.deepStrictEqual([o.result, q.currentTrack.title], ["Skipped Song", "Next"]);
  await run("music/music_control.js", { server: guild, action: "volume", volume: 500 });
  assert.strictEqual(q.volume, 200, "volume is capped");
  await run("music/music_control.js", { server: guild, action: "repeat", repeat: "queue" });
  assert.strictEqual(q.repeatMode, 2);
  o = await run("music/music_control.js", { server: guild, action: "remove", position: 1 });
  assert.strictEqual(o.track.title, "P1");
  await assert.rejects(run("music/music_control.js", { server: guild, action: "jump", position: 9 }), /has no track 9/);
  await assert.rejects(run("music/music_control.js", { server: { id: "other" }, action: "skip" }), /Nothing is playing/);

  // Filters.
  o = await run("music/music_filters.js", { server: guild, action: "on", filter: "nightcore" });
  assert.deepStrictEqual([o.enabled, o.on], [["Nightcore"], true]);
  o = await run("music/music_filters.js", { server: guild, action: "on", filter: "nightcore" });
  assert.deepStrictEqual(o.enabled, ["Nightcore"], "turning on twice keeps it on");
  o = await run("music/music_filters.js", { server: guild, action: "clear" });
  assert.deepStrictEqual(o.enabled, []);

  // Search and lyrics.
  o = await run("music/search_music.js", { query: "Hit", limit: 1 });
  assert.deepStrictEqual([o.count, o.choices[0]], [1, { name: "Hit · 3:00", value: "https://x/Hit" }]);
  o = await run("music/lyrics.js", { server: guild });
  assert.deepStrictEqual([o.found, o.title, o.lyrics], [true, "A Next", "la la"]);

  // Music Event: a track starting on the player runs it.
  const event = load("music/music_event.js");
  const runs = [];
  event.listen({ fields: { kind: "playerStart" }, discord: { music }, log: console }, (x) => runs.push(x));
  await new Promise((r) => setImmediate(r));
  q.setMetadata({ channel: { id: "text" } });
  player.events.emit("playerStart", q, track("Now", { requestedBy: { id: "u" } }));
  assert.deepStrictEqual([runs[0].title, runs[0].channel.id, runs[0].requestedBy.id, runs[0].server], ["Now", "text", "u", guild]);

  console.log("discord music test passed");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
