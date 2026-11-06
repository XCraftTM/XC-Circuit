// Runtime library for the Discord Bot mode: creates the discord.js client with the
// intents the project's event blocks need, logs in, and registers slash commands.
// Blocks reach it as ctx.discord: the client, and the music player (discord-player)
// that the music blocks share.

module.exports = {
  id: "discord",

  setup(rt) {
    const { Client, GatewayIntentBits, Partials } = rt.require("discord.js");
    const names = rt.intents();
    const unknown = names.filter((n) => !(n in GatewayIntentBits));
    if (unknown.length) rt.log.warn(`Unknown intents ignored: ${unknown.join(", ")}`);
    const intents = names.filter((n) => n in GatewayIntentBits).map((n) => GatewayIntentBits[n]);
    if (!intents.length) intents.push(GatewayIntentBits.Guilds);
    const sharding = shardOptions(rt.setting("shards"));
    const client = new Client({ intents, partials: [Partials.Channel, Partials.Message, Partials.Reaction], ...sharding });
    rt.log.info(`Intents: ${names.length ? names.join(", ") : "Guilds"}`);
    if (sharding.shards) rt.log.info(`Shards: ${sharding.shardCount ?? "chosen by Discord"}`);
    return { client, music: createMusic(rt, client), explainError };
  },

  async start(rt, api) {
    const { Events } = rt.require("discord.js");
    const token = rt.secret("token");
    if (!token) {
      throw new Error("No bot token set. Add it in Bot › Settings (or DISCORD_TOKEN in .env for exported bots).");
    }
    api.client.once(Events.ClientReady, async (client) => {
      rt.log.success(`Logged in as ${client.user.tag} · ${client.guilds.cache.size} servers`);
      try {
        await registerCommands(rt, client);
      } catch (e) {
        rt.log.error(`Registering slash commands failed: ${e.message}`);
      }
    });
    try {
      await api.client.login(token);
    } catch (e) {
      const hint = /disallowed intents/i.test(e.message)
        ? " Turn on the privileged intents for your bot in the Discord developer portal (Bot › Privileged Gateway Intents)."
        : "";
      throw new Error(`Login failed: ${e.message}.${hint}`);
    }
  },
};

/**
 * Client options for the Shards setting: auto lets Discord choose, a number
 * over 1 runs that many; all in this process (discord.js's internal sharding).
 */
function shardOptions(setting) {
  const s = String(setting ?? "").trim().toLowerCase();
  if (s === "auto") return { shards: "auto" };
  const n = Number(s);
  if (Number.isInteger(n) && n > 1) return { shards: [...Array(n).keys()], shardCount: n };
  return {};
}

module.exports.shardOptions = shardOptions;

/** Discord's names for permissions, where the flag name doesn't read well. */
const PERMISSION_NAMES = {
  ManageGuild: "Manage Server",
  ManageGuildExpressions: "Manage Expressions",
  ModerateMembers: "Timeout Members",
  CreateInstantInvite: "Create Invite",
  ViewChannel: "View Channels",
  UseExternalStickers: "Use External Stickers",
  SendPolls: "Create Polls",
};
const permissionName = (flag) => PERMISSION_NAMES[flag] || String(flag).replace(/([a-z])([A-Z])/g, "$1 $2");

/**
 * The permissions a block needs (its `requires.permissions`): a list, or a
 * list per value of one of its fields ({ action: { kick: [...] } }).
 */
function neededPermissions(def, values) {
  const p = def?.permissions;
  if (Array.isArray(p)) return p;
  if (p && typeof p === "object") {
    for (const [field, byValue] of Object.entries(p)) {
      const list = byValue?.[values?.[field]];
      if (Array.isArray(list)) return list;
    }
  }
  return [];
}

/**
 * Discord's "Missing Permissions" and "Missing Access" errors in plain words:
 * which permission the bot needs here, and where to give it.
 */
function explainError(err, def, values) {
  const code = Number(err?.code);
  if (code === 50001) {
    return "The bot can't see this channel or server. Give its role the View Channels permission there (Server Settings › Roles, or the channel's permissions).";
  }
  if (code !== 50013) return null;
  const needed = neededPermissions(def, values).map(permissionName);
  const what = needed.length
    ? `The bot is missing a permission for this. It needs ${needed.length > 1 ? "these" : "this"}: ${needed.join(", ")}.`
    : "The bot is missing a permission for this.";
  return `${what} Give its role ${needed.length > 1 ? "them" : "it"} in Server Settings › Roles (or in the channel's permissions), and keep its role above the roles of the members it changes.`;
}
module.exports.explainError = explainError;
module.exports.musicQuery = musicQuery;
module.exports.neededPermissions = neededPermissions;

/**
 * The music player the music blocks share, made with discord-player the first
 * time one needs it (its packages are only installed when a music block is
 * used). Music Player Ready chooses the sources and defaults; credentials come
 * from Bot › Settings › Music.
 */
/**
 * What a search may look for: web addresses and words, and files only in
 * the bot's files folder (data/files, where the File blocks work). A path
 * anywhere else would let whoever types the query make the bot play (and
 * find out about) files on its computer.
 */
function musicQuery(dataDir, text) {
  const q = String(text ?? "").trim();
  const path = require("path");
  const fs = require("fs");
  // C:\…, \\server\…, /…, ~/…, ./…, ../…, file:…
  const looksLikePath = /^(file:|[a-zA-Z]:[\\/]|[\\/]|~|\.\.?([\\/]|$))/.test(q);
  const web = /^[a-z][a-z0-9+.-]*:\/\//i.test(q);
  if (looksLikePath || (!web && fs.existsSync(q))) {
    throw new Error("Music plays web addresses and searches, not files on the bot's computer. For a file in the bot's files folder (data/files), give just its name, such as song.mp3.");
  }
  // A file the bot keeps, by its name in data/files.
  const root = path.join(dataDir, "files");
  const inFiles = path.resolve(root, q);
  const inside = inFiles.startsWith(root + path.sep);
  if (!web && q && inside && fs.existsSync(inFiles) && fs.statSync(inFiles).isFile()) return inFiles;
  return q;
}

function createMusic(rt, client) {
  let options = {};
  let ready = null;
  const engines = {};
  const music = {
    /** Sources and queue defaults, set by Music Player Ready before the player starts. */
    configure(o) {
      options = { ...options, ...o };
    },
    /** The discord-player Player. Waits a moment first, so every block's settings are in. */
    get() {
      if (!ready) {
        ready = new Promise((r) => setImmediate(r)).then(start);
        ready.catch(() => (ready = null));
      }
      return ready;
    },
    /** Queue options for a new queue, from the defaults. */
    nodeOptions(metadata) {
      const d = options;
      return {
        metadata,
        volume: d.volume ?? 80,
        leaveOnEmpty: d.leaveOnEmpty ?? true,
        leaveOnEmptyCooldown: (d.leaveOnEmptySeconds ?? 60) * 1000,
        leaveOnEnd: d.leaveOnEnd ?? true,
        leaveOnEndCooldown: (d.leaveOnEndSeconds ?? 60) * 1000,
        selfDeaf: d.selfDeaf ?? true,
      };
    },
    /** A server's queue, or null when nothing plays there. */
    async queue(server) {
      const player = await music.get();
      return player.nodes.get(server) ?? null;
    },
    /** Like queue(), but fails with a clear message. */
    async needQueue(server) {
      if (!server) throw new Error("Connect a server");
      const q = await music.queue(server);
      if (!q) throw new Error("Nothing is playing on this server");
      return q;
    },
    /** What a search may look for: see musicQuery. */
    query(text) {
      return musicQuery(rt.opts.dataDir || process.cwd(), text);
    },
    /** The search engine for a source name (youtube, spotify …), or auto. */
    engine(source) {
      return engines[source] ? `ext:${engines[source]}` : "auto";
    },
    info: trackInfo,
  };

  async function start() {
    const o = { youtube: true, spotify: true, soundcloud: true, deezer: false, appleMusic: false, tts: false, ttsLanguage: "en", ...options };
    if (!process.env.FFMPEG_PATH) {
      try {
        process.env.FFMPEG_PATH = rt.require("ffmpeg-static");
      } catch {
        // ffmpeg from the PATH, if there is one.
      }
    }
    const { Player } = rt.require("discord-player");
    const player = new Player(client);
    const loaded = [];
    const use = async (name, key, load, opts) => {
      try {
        const extractor = load();
        await player.extractors.register(extractor, opts);
        engines[key] = extractor.identifier;
        loaded.push(name);
      } catch (e) {
        rt.log.warn(`Music: ${name} couldn't be loaded: ${e.message}`);
      }
    };
    const secret = (name) => rt.secret(name) || undefined;
    if (o.youtube) await use("YouTube", "youtube", () => rt.require("discord-player-youtubei").YoutubeExtractor, { cookie: secret("youtubeCookie") });
    if (o.spotify) {
      const clientId = String(rt.setting("spotifyClientId") || "").trim() || undefined;
      await use("Spotify", "spotify", () => rt.require("discord-player-spotify").SpotifyExtractor, { clientId, clientSecret: secret("spotifyClientSecret") });
    }
    if (o.soundcloud) await use("SoundCloud", "soundcloud", () => rt.require("discord-player-soundcloud").SoundcloudExtractor, {});
    if (o.deezer) {
      const arl = secret("deezerArl");
      if (arl) await use("Deezer", "deezer", () => rt.require("discord-player-deezer").DeezerExtractor, { arl, decryptionKey: secret("deezerDecryptionKey") });
      else rt.log.warn("Music: Deezer needs its ARL cookie in Bot › Settings › Music");
    }
    if (o.appleMusic) await use("Apple Music", "appleMusic", () => rt.require("discord-player-applemusic").AppleMusicExtractor, {});
    if (o.tts) await use("Text to speech", "tts", () => rt.require("discord-player-tts").TTSExtractor, { language: o.ttsLanguage || "en", slow: false });
    const base = rt.require("@discord-player/extractor");
    await use("Files and links", "files", () => base.AttachmentExtractor, {});
    await use("Vimeo", "vimeo", () => base.VimeoExtractor, {});
    await use("Reverbnation", "reverbnation", () => base.ReverbnationExtractor, {});
    player.events.on("error", (queue, e) => rt.log.error(`Music (${queue.guild?.name ?? "?"}): ${e.message}`));
    player.events.on("playerError", (queue, e, track) => rt.log.error(`Music: couldn't play ${track?.title ?? "a track"}: ${e.message}`));
    rt.log.info(`Music ready: ${loaded.join(", ")}`);
    player.xcSources = loaded;
    return player;
  }
  return music;
}

/** What blocks show of a discord-player track. */
function trackInfo(track) {
  if (!track) return null;
  return {
    title: track.cleanTitle || track.title || "",
    author: track.author || "",
    url: track.url || "",
    duration: track.duration || "",
    durationMs: track.durationMS ?? 0,
    thumbnail: track.thumbnail || "",
    source: track.source || track.extractor?.identifier || "",
    live: Boolean(track.live),
    requestedBy: track.requestedBy ?? null,
    track,
  };
}

module.exports.trackInfo = trackInfo;
module.exports.createMusic = createMusic;

// Discord's numbers for option types, places, install types and channel kinds.
const OPTION_TYPES = { string: 3, integer: 4, boolean: 5, user: 6, channel: 7, role: 8, mentionable: 9, number: 10, attachment: 11 };
const CONTEXTS = { guild: 0, botDm: 1, privateChannel: 2 };
const INSTALLS = { guild: 0, user: 1 };
const CHANNEL_TYPES = { text: 0, voice: 2, category: 4, announcement: 5, announcementThread: 10, publicThread: 11, privateThread: 12, stage: 13, forum: 15, media: 16 };

const describe = (text) => {
  const t = String(text || "").trim();
  return t ? [...t].slice(0, 100).join("") : "A command made with XC Circuit";
};
const words = (c) => String(c.name || "").trim().split(/\s+/).filter(Boolean);

/** Translations ({ de: { name, description } }) as Discord's name and description localizations. */
function putLocales(out, locales, withDescription = true) {
  const pick = (key) => Object.fromEntries(Object.entries(locales || {}).filter(([, l]) => l && l[key]).map(([code, l]) => [code, l[key]]));
  const names = pick("name");
  if (Object.keys(names).length) out.name_localizations = names;
  if (withDescription) {
    const descriptions = pick("description");
    if (Object.keys(descriptions).length) out.description_localizations = descriptions;
  }
  return out;
}

/** A Slash Command Builder option in the shape Discord's API expects. */
function toDiscordOption(o) {
  const numeric = o.type === "integer" || o.type === "number";
  const text = o.type === "string";
  const out = { type: OPTION_TYPES[o.type] || 3, name: o.name, description: describe(String(o.description || "").trim() ? o.description : o.name) };
  putLocales(out, o.locales);
  out.required = Boolean(o.required);
  if ((o.choices || []).length && (text || numeric)) {
    out.choices = o.choices.slice(0, 25).map((ch) => putLocales({ name: String(ch.name), value: numeric ? Number(ch.value) || 0 : String(ch.value ?? "") }, ch.locales, false));
  }
  if (numeric && o.min != null) out.min_value = Number(o.min);
  if (numeric && o.max != null) out.max_value = Number(o.max);
  // Older projects kept a text option's longest length in max.
  const maxLength = o.maxLength ?? (text ? o.max : undefined);
  if (text && o.minLength != null) out.min_length = Number(o.minLength);
  if (text && maxLength != null) out.max_length = Math.max(1, Number(maxLength));
  if (o.type === "channel" && (o.channelTypes || []).length) out.channel_types = o.channelTypes.map((t) => CHANNEL_TYPES[t]).filter((n) => n !== undefined);
  if (o.autocomplete && !out.choices && (text || numeric)) out.autocomplete = true;
  return out;
}

// Discord requires required options before optional ones.
const optionsOf = (c) => (c.options || []).slice().sort((a, b) => Number(Boolean(b.required)) - Number(Boolean(a.required))).map(toDiscordOption);

const sub = (c, type, name, options) => putLocales({ type, name, description: describe(c.description), options }, c.locales);

/**
 * The Slash Command Builder's commands in the shape Discord's API expects.
 * Names with spaces are subcommands: "ticket open" and "ticket close" become
 * /ticket with two subcommands, and three words make a subcommand group. A
 * name others continue ("ticket", "ticket admin") gives that command or group
 * its description and translations. Who can use a command and where come
 * from the command itself, or its first subcommand.
 * `permissionBits` turns a permission name into its bit string.
 * The editor builds the same JSON (slash.rs); test/fixtures/slash.json checks both.
 */
function toDiscordCommands(list, permissionBits = () => undefined) {
  list = list.filter((c) => words(c).length);
  const roots = [...new Set(list.map((c) => words(c)[0]))];
  const isHeader = (name) => list.some((o) => o.name.startsWith(`${name} `));
  return roots.map((root) => {
    const entries = list.filter((c) => words(c)[0] === root);
    const me = entries.find((c) => words(c).length === 1);
    const nested = entries.filter((c) => words(c).length === 2 && !isHeader(c.name)).map((c) => sub(c, 1, words(c)[1], optionsOf(c)));
    const groups = [...new Set(entries.filter((c) => words(c).length >= 3).map((c) => words(c)[1]))];
    for (const g of groups) {
      const subs = entries.filter((c) => words(c).length >= 3 && words(c)[1] === g).map((c) => sub(c, 1, words(c)[2], optionsOf(c)));
      const header = entries.find((c) => c.name === `${root} ${g}`) || { description: g };
      nested.push(sub(header, 2, g, subs));
    }
    const settings = me || entries[0];
    const description = me && String(me.description || "").trim() ? describe(me.description) : describe(nested[0] && nested[0].description);
    const out = { name: root, description };
    if (me) putLocales(out, me.locales);
    out.options = nested.length ? nested : me ? optionsOf(me) : [];
    const names = (settings.permissions || []).length ? settings.permissions : settings.permission ? [settings.permission] : [];
    const bits = names.map(permissionBits).filter((b) => b !== undefined).reduce((all, b) => all | BigInt(b), 0n);
    if (bits) out.default_member_permissions = bits.toString();
    // Older projects had DMs on or off.
    const contexts = (settings.contexts || []).length ? settings.contexts : settings.dm ? ["guild", "botDm"] : ["guild"];
    out.contexts = contexts.map((c) => CONTEXTS[c]).filter((n) => n !== undefined);
    if (!out.contexts.length) out.contexts = [0];
    if ((settings.installs || []).length) out.integration_types = settings.installs.map((c) => INSTALLS[c]).filter((n) => n !== undefined);
    if (settings.nsfw) out.nsfw = true;
    return out;
  });
}

/** One Builder command (kept for older callers): its root command alone. */
function toDiscordCommand(c) {
  return toDiscordCommands([c])[0];
}

module.exports.toDiscordCommands = toDiscordCommands;
module.exports.toDiscordCommand = toDiscordCommand;
module.exports.registrationPlan = registrationPlan;

/**
 * What to send Discord, given the commands, where they go ("global" or a
 * server id) and what was registered last time ({ scope, hash }, or null).
 * Unchanged commands aren't sent again, a list that became empty is still
 * sent (so deleted commands go away), and moving between the dev server and
 * global clears the old place, so commands don't show up twice.
 */
function registrationPlan(list, scope, last) {
  const hash = require("crypto").createHash("sha256").update(JSON.stringify(list)).digest("hex");
  const steps = [];
  if (last && last.scope !== scope && last.count) steps.push({ scope: last.scope, list: [] });
  const changed = !last || last.scope !== scope || last.hash !== hash;
  if (changed && (list.length || (last && last.scope === scope && last.count))) steps.push({ scope, list });
  return { steps, record: { scope, hash, count: list.length } };
}

/** Registers every Slash Command block's command, on the dev server or globally. */
async function registerCommands(rt, client) {
  const commands = new Map();
  // Commands from the Slash Command Builder, with their options and subcommands.
  const { PermissionFlagsBits } = rt.require("discord.js");
  const bits = (name) => (name in PermissionFlagsBits ? PermissionFlagsBits[name].toString() : undefined);
  for (const c of toDiscordCommands((rt.bundle.project.data && rt.bundle.project.data.slashCommands) || [], bits)) {
    commands.set(c.name, c);
  }
  // Commands only named on a block still register, without options.
  for (const node of rt.nodesOf("discord.on_slash_command")) {
    const name = String(node.inline().command || "").trim().toLowerCase().split(/\s+/)[0];
    if (name && !commands.has(name)) commands.set(name, { name, description: "A command made with XC Circuit" });
  }
  // Context Menu Used blocks add right-click entries (type 2: users, 3: messages).
  for (const node of rt.nodesOf("discord.on_context_menu")) {
    const v = node.inline();
    const name = String(v.name || "").trim().slice(0, 32);
    const type = v.target === "message" ? 3 : 2;
    if (name) commands.set(`menu:${type}:${name}`, { name, type });
  }
  const list = [...commands.values()];
  const devServer = String(rt.setting("devServer") || "").trim();
  // What was registered last time, kept with the bot's saved data.
  const fs = require("fs");
  const file = require("path").join(rt.opts.dataDir, "registered-commands.json");
  let last = null;
  try {
    last = JSON.parse(fs.readFileSync(file, "utf8"));
  } catch {}
  const plan = registrationPlan(list, devServer || "global", last);
  for (const step of plan.steps) {
    const where = step.scope === "global" ? "globally" : "on the dev server";
    if (step.scope === "global") await client.application.commands.set(step.list);
    else await client.application.commands.set(step.list, step.scope);
    rt.log.info(step.list.length ? `Registered ${step.list.length} slash commands ${where}` : `Removed the slash commands ${where}`);
  }
  if (!plan.steps.length && list.length) rt.log.info(`Slash commands unchanged (${list.length})`);
  try {
    fs.mkdirSync(rt.opts.dataDir, { recursive: true });
    fs.writeFileSync(file, JSON.stringify(plan.record) + "\n");
  } catch (e) {
    rt.log.warn(`Couldn't remember the registered slash commands: ${e.message}`);
  }
}
