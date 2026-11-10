// XC Circuit Host: the container people run themselves.
//
// One port, two modes:
//   relay  passes the messages of a live session from one app to the others
//          and keeps nothing. This is all a public relay does.
//   full   (the default) also keeps projects (as Git repositories, so the
//          app has one way to keep in step), the devices that were paired
//          with it, and their secrets, which arrive encrypted and are never
//          readable here.
//
// It speaks plain HTTP and WebSocket on its port and expects a reverse
// proxy (Nginx Proxy Manager, Caddy, Traefik …) to add HTTPS in front.
// No dependencies: Node.js 24 has what it needs (node:sqlite, crypto).
"use strict";

const http = require("node:http");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");

const VERSION = "0.3.0";
/// Raised when the messages below change in a way older apps can't follow.
const PROTOCOL = 1;

const LIMITS = {
  message: 256 * 1024, // one message of a session, as sent
  guests: 16, // in one room
  roomsPerAddress: 4,
  connectionsPerAddress: 32,
  vaultBlob: 64 * 1024,
  keyBlob: 8 * 1024,
  waitingKeys: 64, // per device
  codeMinutes: 10,
  keyDays: 30, // a key nobody fetched
  quietSeconds: 75, // without an answer to a ping
  bots: 20, // that the host runs
  botFile: 30 * 1024 * 1024, // one file of a bot
  botLines: 300, // of a bot's console kept for whoever looks later
};

function settings(env = process.env) {
  const mode = (env.XC_HOST_MODE || "full").toLowerCase();
  if (mode !== "full" && mode !== "relay") throw new Error(`XC_HOST_MODE is "${env.XC_HOST_MODE}": it must be full or relay.`);
  const port = Number(env.XC_HOST_PORT || 7421);
  if (!Number.isInteger(port) || port < 0 || port > 65535) throw new Error(`XC_HOST_PORT is "${env.XC_HOST_PORT}": it must be a port number.`);
  return { mode, port, data: env.XC_HOST_DATA || "/data", trusted: (env.XC_HOST_TRUSTED_PROXY || "").split(",").map((s) => s.trim()).filter(Boolean) };
}

// ---- WebSocket, as much of it as the app uses --------------------------------

const GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

function frame(opcode, payload) {
  const length = payload.length;
  const head = length < 126 ? Buffer.from([0x80 | opcode, length]) : length < 65536 ? Buffer.from([0x80 | opcode, 126, length >> 8, length & 255]) : Buffer.concat([Buffer.from([0x80 | opcode, 127]), (() => { const b = Buffer.alloc(8); b.writeBigUInt64BE(BigInt(length)); return b; })()]);
  return Buffer.concat([head, payload]);
}

/// One connected app. `onMessage(text)` gets each text message; `onClose()` once.
class Socket {
  constructor(socket, address) {
    this.socket = socket;
    this.address = address;
    this.buffer = Buffer.alloc(0);
    this.parts = [];
    this.partsSize = 0;
    this.closed = false;
    this.heard = Date.now();
    this.onMessage = () => {};
    this.onClose = () => {};
    socket.on("data", (chunk) => this.read(chunk));
    socket.on("close", () => this.end());
    socket.on("error", () => this.end());
  }

  send(message) {
    if (!this.closed) this.socket.write(frame(1, Buffer.from(JSON.stringify(message))));
  }

  ping() {
    if (!this.closed) this.socket.write(frame(9, Buffer.alloc(0)));
  }

  close(code = 1000, why = "") {
    if (this.closed) return;
    const body = Buffer.concat([Buffer.from([code >> 8, code & 255]), Buffer.from(why)]);
    this.socket.write(frame(8, body));
    this.socket.end();
    this.end();
  }

  end() {
    if (this.closed) return;
    this.closed = true;
    this.socket.destroy();
    this.onClose();
  }

  read(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (!this.closed) {
      if (this.buffer.length < 2) return;
      const [first, second] = this.buffer;
      const opcode = first & 15;
      let length = second & 127;
      let at = 2;
      if (length === 126) {
        if (this.buffer.length < 4) return;
        length = this.buffer.readUInt16BE(2);
        at = 4;
      } else if (length === 127) {
        if (this.buffer.length < 10) return;
        const long = this.buffer.readBigUInt64BE(2);
        if (long > BigInt(LIMITS.message * 2)) return this.close(1009, "too big");
        length = Number(long);
        at = 10;
      }
      // Apps mask what they send; anything else isn't one.
      if (!(second & 128)) return this.close(1002, "not masked");
      if (length + this.partsSize > LIMITS.message * 2) return this.close(1009, "too big");
      if (this.buffer.length < at + 4 + length) return;
      const mask = this.buffer.subarray(at, at + 4);
      const payload = Buffer.from(this.buffer.subarray(at + 4, at + 4 + length));
      for (let i = 0; i < payload.length; i++) payload[i] ^= mask[i & 3];
      this.buffer = this.buffer.subarray(at + 4 + length);
      this.heard = Date.now();
      if (opcode === 8) return this.close();
      if (opcode === 9) {
        this.socket.write(frame(10, payload));
        continue;
      }
      if (opcode === 10) continue;
      if (opcode === 2) return this.close(1003, "text only");
      this.parts.push(payload);
      this.partsSize += payload.length;
      if (first & 128) {
        const text = Buffer.concat(this.parts).toString("utf8");
        this.parts = [];
        this.partsSize = 0;
        this.onMessage(text);
      }
    }
  }
}

// ---- The store: what full mode keeps -----------------------------------------
//
// Everything that touches the database is in here, so another database could
// take its place.

const hash = (text) => crypto.createHash("sha256").update(text).digest("hex");
const random = (bytes) => crypto.randomBytes(bytes).toString("hex");
/// A code to read out or type: 8 letters and digits without the ones that look alike.
function newCode() {
  const letters = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  const pick = Array.from(crypto.randomBytes(8), (b) => letters[b % letters.length]).join("");
  return `${pick.slice(0, 4)}-${pick.slice(4)}`;
}

class Store {
  constructor(dir) {
    const { DatabaseSync } = require("node:sqlite");
    this.db = new DatabaseSync(path.join(dir, "host.sqlite"));
    this.db.exec(`
      PRAGMA journal_mode = WAL;
      CREATE TABLE IF NOT EXISTS devices (id TEXT PRIMARY KEY, name TEXT NOT NULL, token TEXT NOT NULL UNIQUE, key TEXT NOT NULL DEFAULT '', created INTEGER NOT NULL, seen INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS codes (code TEXT PRIMARY KEY, expires INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS vault (project TEXT PRIMARY KEY, blob TEXT NOT NULL, rev INTEGER NOT NULL, updated INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS keys (id INTEGER PRIMARY KEY AUTOINCREMENT, for_device TEXT NOT NULL, from_device TEXT NOT NULL, blob TEXT NOT NULL, created INTEGER NOT NULL);
    `);
  }

  close() {
    this.db.close();
  }

  devices() {
    return this.db.prepare("SELECT id, name, key, created, seen FROM devices ORDER BY created").all();
  }

  /// A code that lets one device in, good for a few minutes.
  invite() {
    const code = newCode();
    this.db.prepare("DELETE FROM codes WHERE expires < ?").run(Date.now());
    this.db.prepare("INSERT INTO codes (code, expires) VALUES (?, ?)").run(hash(code), Date.now() + LIMITS.codeMinutes * 60_000);
    return code;
  }

  /// Lets a device in with a code, once. Gives its id and its token, which
  /// is only kept here as a hash.
  pair(code, name, key) {
    const row = this.db.prepare("SELECT expires FROM codes WHERE code = ?").get(hash(String(code).trim().toUpperCase()));
    if (!row || row.expires < Date.now()) return null;
    this.db.prepare("DELETE FROM codes WHERE code = ?").run(hash(String(code).trim().toUpperCase()));
    const device = { id: random(8), token: random(32) };
    const now = Date.now();
    this.db.prepare("INSERT INTO devices (id, name, token, key, created, seen) VALUES (?, ?, ?, ?, ?, ?)").run(device.id, String(name || "Device").slice(0, 60), hash(device.token), String(key || "").slice(0, 200), now, now);
    return device;
  }

  /// The device a token belongs to.
  device(token) {
    if (typeof token !== "string" || !token) return null;
    const row = this.db.prepare("SELECT id, name FROM devices WHERE token = ?").get(hash(token));
    if (row) this.db.prepare("UPDATE devices SET seen = ? WHERE id = ?").run(Date.now(), row.id);
    return row || null;
  }

  remove(id) {
    this.db.prepare("DELETE FROM keys WHERE for_device = ?").run(id);
    return this.db.prepare("DELETE FROM devices WHERE id = ?").run(id).changes > 0;
  }

  vault(project) {
    return this.db.prepare("SELECT blob, rev FROM vault WHERE project = ?").get(project) || null;
  }

  /// Keeps a project's secrets, as the devices encrypted them. `rev` is the
  /// one the device last saw: a newer one here means another device was
  /// faster, and nothing is written.
  putVault(project, blob, rev) {
    const now = this.vault(project);
    if (now && now.rev !== rev) return null;
    const next = (now ? now.rev : 0) + 1;
    this.db.prepare("INSERT INTO vault (project, blob, rev, updated) VALUES (?, ?, ?, ?) ON CONFLICT(project) DO UPDATE SET blob = excluded.blob, rev = excluded.rev, updated = excluded.updated").run(project, blob, next, Date.now());
    return next;
  }

  /// A key one device wrapped for another, kept until that one fetches it.
  putKey(forDevice, fromDevice, blob) {
    this.db.prepare("DELETE FROM keys WHERE created < ?").run(Date.now() - LIMITS.keyDays * 86_400_000);
    if (!this.db.prepare("SELECT 1 FROM devices WHERE id = ?").get(forDevice)) return false;
    if (this.db.prepare("SELECT COUNT(*) AS n FROM keys WHERE for_device = ?").get(forDevice).n >= LIMITS.waitingKeys) return false;
    this.db.prepare("INSERT INTO keys (for_device, from_device, blob, created) VALUES (?, ?, ?, ?)").run(forDevice, fromDevice, blob, Date.now());
    return true;
  }

  takeKeys(forDevice) {
    const rows = this.db.prepare("SELECT from_device AS \"from\", blob FROM keys WHERE for_device = ? ORDER BY id").all(forDevice);
    this.db.prepare("DELETE FROM keys WHERE for_device = ?").run(forDevice);
    return rows;
  }
}

// ---- The host ------------------------------------------------------------------

const PROJECT = /^[a-z0-9][a-z0-9-]{0,63}$/;

async function start(options = settings()) {
  const { mode } = options;
  let store = null;
  let projects = null;
  let setupCode = null;
  const log = options.log || ((line) => console.log(line));
  if (mode === "full") {
    // Refused rather than written into the container, where it would be
    // gone with the next update.
    const dir = options.data;
    try {
      fs.accessSync(dir, fs.constants.R_OK | fs.constants.W_OK);
      if (!fs.statSync(dir).isDirectory()) throw new Error("not a folder");
    } catch {
      throw new Error(`The data folder ${dir} isn't there or can't be written to. Give the container a volume there (see the docs), or start it with XC_HOST_MODE=relay to only pass live sessions on.`);
    }
    store = new Store(dir);
    projects = path.join(dir, "projects");
    fs.mkdirSync(projects, { recursive: true });
    if (store.devices().length === 0) {
      // No password to start with: the first device comes in with this.
      setupCode = store.invite();
      log(`No device is paired yet. Pair the first one in XC Circuit with this code (good for ${LIMITS.codeMinutes} minutes; restart for a new one): ${setupCode}`);
    }
  }

  const rooms = new Map(); // room → { host: Socket, guests: Map<id, Socket> }
  const sockets = new Set();
  const perAddress = new Map();
  const count = (address, by) => {
    const n = (perAddress.get(address) || 0) + by;
    if (n > 0) perAddress.set(address, n);
    else perAddress.delete(address);
    return n;
  };

  /// The app's address: the proxy's word for it, when the proxy is one we
  /// were told to trust.
  const addressOf = (req) => {
    const direct = (req.socket.remoteAddress || "").replace(/^::ffff:/, "");
    const isPrivate = /^(127\.|10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.|::1$|f[cd])/i.test(direct);
    const trusted = options.trusted.includes(direct) || (options.trusted.includes("private") && isPrivate);
    const forwarded = String(req.headers["x-forwarded-for"] || "").split(",")[0].trim();
    return trusted && forwarded ? forwarded : direct;
  };

  const status = () => ({ name: "xc-circuit-host", version: VERSION, protocol: PROTOCOL, mode, can: mode === "full" ? ["relay", "projects", "vault", "bots"] : ["relay"] });

  const server = http.createServer((req, res) => {
    const url = new URL(req.url, "http://host");
    if (req.method === "GET" && (url.pathname === "/status" || url.pathname === "/")) {
      res.writeHead(200, { "Content-Type": "application/json", "Cache-Control": "no-store" });
      return res.end(JSON.stringify(status()));
    }
    if (mode === "full" && url.pathname.startsWith("/git/")) return git(req, res, url);
    if (mode === "full" && url.pathname.startsWith("/api/client/")) return panel(req, res, url);
    res.writeHead(404, { "Content-Type": "text/plain" });
    res.end("Not found\n");
  });

  // Projects: Git over HTTP, by Git's own program. The password is a
  // paired device's token; the name beside it doesn't matter.
  function git(req, res, url) {
    const [, id, rest] = url.pathname.match(/^\/git\/([^/]+)\.git(\/.*)?$/) || [];
    const basic = /^Basic (.+)$/.exec(req.headers.authorization || "");
    const token = basic ? Buffer.from(basic[1], "base64").toString("utf8").split(":").slice(1).join(":") : "";
    const device = store.device(token);
    if (!device) {
      res.writeHead(401, { "WWW-Authenticate": 'Basic realm="XC Circuit Host"', "Content-Type": "text/plain" });
      return res.end("Pair this device first.\n");
    }
    if (!id || !PROJECT.test(id)) {
      res.writeHead(404, { "Content-Type": "text/plain" });
      return res.end("A project's name is lowercase letters, digits and dashes.\n");
    }
    const repo = path.join(projects, `${id}.git`);
    if (!fs.existsSync(repo)) {
      const made = spawnSync("git", ["init", "--bare", "--initial-branch=main", repo], { encoding: "utf8" });
      if (made.status !== 0) {
        res.writeHead(500, { "Content-Type": "text/plain" });
        return res.end("Git isn't available on this host.\n");
      }
      spawnSync("git", ["-C", repo, "config", "http.receivepack", "true"]);
    }
    const env = {
      PATH: process.env.PATH,
      GIT_PROJECT_ROOT: projects,
      GIT_HTTP_EXPORT_ALL: "1",
      REQUEST_METHOD: req.method,
      PATH_INFO: `/${id}.git${rest || ""}`,
      QUERY_STRING: url.search.slice(1),
      CONTENT_TYPE: req.headers["content-type"] || "",
      REMOTE_USER: device.id,
      REMOTE_ADDR: addressOf(req),
      GIT_HTTP_MAX_REQUEST_BUFFER: "100M",
    };
    if (req.headers["content-length"]) env.CONTENT_LENGTH = req.headers["content-length"];
    if (req.headers["content-encoding"]) env.HTTP_CONTENT_ENCODING = req.headers["content-encoding"];
    if (req.headers["git-protocol"]) env.GIT_PROTOCOL = req.headers["git-protocol"];
    const child = spawn("git", ["http-backend"], { env });
    req.pipe(child.stdin);
    child.stdin.on("error", () => {});
    // What Git complains about goes to the log: it is the only place that
    // says why storing a project didn't work.
    let complaint = "";
    child.stderr.on("data", (chunk) => {
      if (complaint.length < 4000) complaint += chunk.toString("utf8");
    });
    req.on("aborted", () => log(`Git ${req.method} ${id}.git${rest || ""}: the connection was cut before the request was complete`));
    // Nothing goes back before the whole request is here. Git starts its
    // answer early, and a reverse proxy that sees an answer stops sending
    // the rest of an upload and cuts the connection (Nginx does): storing
    // anything bigger than its buffers failed behind one.
    let head = Buffer.alloc(0);
    let sent = false;
    let arrived = false;
    let ended = false;
    const answer = () => {
      if (!arrived) return;
      if (!sent) {
        const end = head.indexOf("\r\n\r\n");
        if (end < 0) return;
        const headers = {};
        let code = 200;
        for (const line of head.subarray(0, end).toString("latin1").split("\r\n")) {
          const at = line.indexOf(":");
          const name = line.slice(0, at).trim();
          const value = line.slice(at + 1).trim();
          if (name.toLowerCase() === "status") code = parseInt(value, 10) || 200;
          else if (name) headers[name] = value;
        }
        res.writeHead(code, headers);
        res.write(head.subarray(end + 4));
        head = Buffer.alloc(0);
        sent = true;
      } else if (head.length) {
        res.write(head);
        head = Buffer.alloc(0);
      }
    };
    const finish = () => {
      if (!ended || !arrived) return;
      answer();
      if (!sent) res.writeHead(500, { "Content-Type": "text/plain" });
      res.end();
    };
    req.on("end", () => {
      arrived = true;
      answer();
      finish();
    });
    child.stdout.on("data", (chunk) => {
      head = Buffer.concat([head, chunk]);
      answer();
    });
    child.on("error", () => {
      if (!sent) res.writeHead(500, { "Content-Type": "text/plain" });
      sent = true;
      res.end("Git isn't available on this host.\n");
    });
    child.on("close", (code) => {
      if (complaint.trim() || code) log(`Git ${req.method} ${id}.git${rest || ""} ended with ${code}: ${complaint.trim() || "no message"}`);
      ended = true;
      // Git gave up before reading it all: what is still coming is let go.
      if (!arrived) req.resume();
      finish();
      // Something was stored: the other devices hear of it at once, so
      // they can fetch it without anyone pressing anything.
      if (code === 0 && req.method === "POST" && rest === "/git-receive-pack") {
        for (const other of sockets) if (other.device) other.send({ t: "project.changed", project: id, by: device.id });
      }
    });
  }

  // ---- Running bots ----------------------------------------------------------
  //
  // A bot is an exported project (index.js, package.json, .env) in its own
  // folder under bots/. The host installs its packages, starts it with
  // Node.js, keeps what it prints, and starts it again after a restart of
  // the host if it was running. The app reaches all of it the way it
  // reaches a game panel (the Calagopus client API, as far as the app uses
  // it), so Bot › Remote… works with a host as it does with a panel.

  const bots = new Map(); // id → { id, dir, child, state, started, lines, watchers, stopping }
  const botsDir = mode === "full" ? path.join(options.data, "bots") : null;
  const consoleTokens = new Map(); // one-time token → { id, expires }
  if (botsDir) fs.mkdirSync(botsDir, { recursive: true });

  function botOf(id) {
    let bot = bots.get(id);
    if (!bot) {
      bot = { id, dir: path.join(botsDir, id), child: null, state: "offline", started: 0, lines: [], watchers: new Set(), stopping: false };
      bots.set(id, bot);
    }
    return bot;
  }

  const noteOf = (bot) => {
    try {
      return JSON.parse(fs.readFileSync(path.join(bot.dir, ".xc-host.json"), "utf8"));
    } catch {
      return {};
    }
  };
  const keepNote = (bot, change) => {
    try {
      fs.writeFileSync(path.join(bot.dir, ".xc-host.json"), JSON.stringify({ ...noteOf(bot), ...change }));
    } catch {
      // The folder is gone: nothing to note.
    }
  };

  function botSays(bot, text) {
    for (const line of String(text).split(/\r?\n/)) {
      if (!line) continue;
      bot.lines.push(line);
      if (bot.lines.length > LIMITS.botLines) bot.lines.shift();
      for (const watcher of bot.watchers) watcher.send({ event: "console output", args: [line] });
    }
  }

  function botState(bot, state) {
    bot.state = state;
    for (const watcher of bot.watchers) watcher.send({ event: "status", args: [state] });
  }

  /// A file's place in the bot's folder, or null for a path that would
  /// leave it. Its own data/ is the bot's alone: uploads never touch it.
  function botPath(bot, file) {
    const parts = String(file || "").split(/[\\/]+/).filter(Boolean);
    if (!parts.length || parts.some((p) => p === ".." || p === "." || p.includes(":")) || parts[0] === "data" || parts[0] === ".xc-host.json") return null;
    return path.join(bot.dir, ...parts);
  }

  function runBot(bot) {
    bot.stopping = false;
    const child = spawn(process.execPath, ["index.js"], { cwd: bot.dir, env: { ...process.env, XC_HOST: "1" }, stdio: ["pipe", "pipe", "pipe"] });
    bot.child = child;
    bot.started = Date.now();
    botState(bot, "running");
    child.stdout.on("data", (chunk) => botSays(bot, chunk.toString("utf8")));
    child.stderr.on("data", (chunk) => botSays(bot, chunk.toString("utf8")));
    child.stdin.on("error", () => {});
    child.on("error", (e) => botSays(bot, `Couldn't start the bot: ${e.message}`));
    child.on("close", (code) => {
      if (bot.child !== child) return;
      bot.child = null;
      botSays(bot, bot.stopping ? "The bot stopped." : `The bot ended${code == null ? "" : ` (${code})`}.`);
      botState(bot, "offline");
      if (bot.again) {
        bot.again = false;
        startBot(bot);
      }
    });
  }

  function startBot(bot) {
    if (bot.child || bot.state === "starting") return;
    if (!fs.existsSync(path.join(bot.dir, "index.js"))) {
      botSays(bot, "There is no bot here yet: upload it first.");
      return;
    }
    keepNote(bot, { wanted: "running" });
    // Its packages, when package.json asks for other ones than last time.
    let wants = "";
    try {
      const pack = JSON.parse(fs.readFileSync(path.join(bot.dir, "package.json"), "utf8"));
      const deps = pack.dependencies || {};
      if (Object.keys(deps).length) wants = hash(JSON.stringify(deps));
    } catch {
      // No package.json, or not one: nothing to install.
    }
    if (!wants || noteOf(bot).installed === wants) return runBot(bot);
    botState(bot, "starting");
    botSays(bot, "Installing the bot's packages …");
    const npm = spawn("npm", ["install", "--omit=dev", "--no-audit", "--no-fund"], { cwd: bot.dir, shell: process.platform === "win32" });
    bot.installing = npm;
    npm.stdout.on("data", (chunk) => botSays(bot, chunk.toString("utf8")));
    npm.stderr.on("data", (chunk) => botSays(bot, chunk.toString("utf8")));
    npm.on("error", (e) => botSays(bot, `npm isn't available on this host: ${e.message}`));
    npm.on("close", (code) => {
      bot.installing = null;
      if (bot.stopping) return botState(bot, "offline");
      if (code !== 0) {
        botSays(bot, "The packages couldn't be installed: the bot wasn't started.");
        return botState(bot, "offline");
      }
      keepNote(bot, { installed: wants });
      runBot(bot);
    });
  }

  function stopBot(bot, hard) {
    keepNote(bot, { wanted: "stopped" });
    bot.stopping = true;
    bot.again = false;
    if (bot.installing) bot.installing.kill();
    const child = bot.child;
    if (!child) return;
    botState(bot, "stopping");
    child.kill(hard ? "SIGKILL" : "SIGTERM");
    // One that doesn't end by itself is ended.
    if (!hard) setTimeout(() => bot.child === child && child.kill("SIGKILL"), 10_000).unref();
  }

  function powerBot(bot, action) {
    if (action === "start") return startBot(bot);
    if (action === "stop") return stopBot(bot, false);
    if (action === "kill") return stopBot(bot, true);
    if (action === "restart") {
      if (!bot.child) return startBot(bot);
      stopBot(bot, false);
      keepNote(bot, { wanted: "running" });
      bot.again = true;
    }
  }

  /// A bot's memory, where the system says (Linux): bytes.
  function botMemory(bot) {
    try {
      const status = fs.readFileSync(`/proc/${bot.child.pid}/status`, "utf8");
      return Number(/VmRSS:\s+(\d+)/.exec(status)[1]) * 1024;
    } catch {
      return 0;
    }
  }

  if (botsDir) {
    // Back after a restart of the host: the bots that were running run again.
    for (const id of fs.readdirSync(botsDir)) {
      if (!PROJECT.test(id)) continue;
      const bot = botOf(id);
      if (noteOf(bot).wanted === "running") startBot(bot);
    }
  }

  // The app's side of a game panel's API, for what the app uses of it.
  function panel(req, res, url) {
    const answer = (code, body) => {
      res.writeHead(code, { "Content-Type": "application/json", "Cache-Control": "no-store" });
      res.end(JSON.stringify(body));
    };
    const bearer = /^Bearer (.+)$/.exec(req.headers.authorization || "");
    const device = store.device(bearer ? bearer[1].trim() : "");
    if (!device) return answer(401, { errors: ["Pair this device with the host first."] });
    const parts = url.pathname.split("/").filter(Boolean).slice(2); // after api/client
    if (parts[0] !== "servers") return answer(404, { errors: ["Not found"] });
    if (parts.length === 1 && req.method === "GET") {
      // Every bot there is, and every project kept here that could be one.
      const names = new Set([...fs.readdirSync(botsDir), ...fs.readdirSync(projects).filter((n) => n.endsWith(".git")).map((n) => n.slice(0, -4))].filter((n) => PROJECT.test(n)));
      const data = [...names].sort().map((id) => ({ uuid: id, uuid_short: id, name: id }));
      return answer(200, { servers: { data, total: data.length } });
    }
    const id = parts[1];
    if (!PROJECT.test(id || "")) return answer(404, { errors: ["A bot's name is lowercase letters, digits and dashes."] });
    if (!bots.has(id) && !fs.existsSync(path.join(botsDir, id)) && fs.readdirSync(botsDir).length >= LIMITS.bots) return answer(409, { errors: [`This host runs at most ${LIMITS.bots} bots.`] });
    const bot = botOf(id);
    const what = parts.slice(2).join("/");
    if (what === "resources" && req.method === "GET") {
      return answer(200, { resources: { state: bot.state, memory_bytes: bot.child ? botMemory(bot) : 0, memory_limit_bytes: 0, cpu_absolute: 0, uptime: bot.child ? Date.now() - bot.started : 0 } });
    }
    if (what === "websocket" && req.method === "GET") {
      const token = random(24);
      consoleTokens.set(token, { id, expires: Date.now() + 60_000 });
      const secure = (req.headers["x-forwarded-proto"] || "").includes("https");
      return answer(200, { url: `${secure ? "wss" : "ws"}://${req.headers.host}/bot-console/${id}`, token });
    }
    if (req.method !== "POST") return answer(404, { errors: ["Not found"] });
    const chunks = [];
    let size = 0;
    req.on("data", (chunk) => {
      size += chunk.length;
      if (size <= LIMITS.botFile) chunks.push(chunk);
    });
    req.on("end", () => {
      if (size > LIMITS.botFile) return answer(413, { errors: ["That file is too big for this host."] });
      const body = Buffer.concat(chunks);
      const json = () => {
        try {
          return JSON.parse(body.toString("utf8"));
        } catch {
          return {};
        }
      };
      try {
        if (what === "power") {
          const action = String(json().action);
          if (!["start", "stop", "restart", "kill"].includes(action)) return answer(422, { errors: ["That isn't something a bot can be told."] });
          powerBot(bot, action);
          return answer(200, {});
        }
        if (what === "command") {
          if (bot.child) bot.child.stdin.write(`${String(json().command || "")}\n`);
          return answer(200, {});
        }
        if (what === "files/write") {
          const to = botPath(bot, url.searchParams.get("file"));
          if (!to) return answer(422, { errors: ["That isn't a place in the bot's folder."] });
          fs.mkdirSync(path.dirname(to), { recursive: true });
          fs.writeFileSync(to, body);
          return answer(200, {});
        }
        if (what === "files/create-directory") {
          const { root, name } = json();
          const to = botPath(bot, `${root || ""}/${name || ""}`);
          if (!to) return answer(422, { errors: ["That isn't a place in the bot's folder."] });
          fs.mkdirSync(to, { recursive: true });
          return answer(200, {});
        }
        return answer(404, { errors: ["Not found"] });
      } catch (e) {
        log(`Bot ${id}, ${what}: ${e && e.message}`);
        return answer(500, { errors: ["The host couldn't do that."] });
      }
    });
  }

  /// A bot's live console, as a game panel's: what it printed lately, then
  /// every line as it comes, its state, and now and then its memory.
  function botConsole(raw, key, address, id) {
    const accept = crypto.createHash("sha1").update(key + GUID).digest("base64");
    raw.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    const socket = new Socket(raw, address);
    sockets.add(socket);
    count(address, 1);
    const bot = botOf(id);
    let allowed = false;
    socket.onMessage = (text) => {
      let message;
      try {
        message = JSON.parse(text);
      } catch {
        return;
      }
      const args = Array.isArray(message.args) ? message.args : [];
      if (message.event === "auth") {
        const token = consoleTokens.get(String(args[0]));
        consoleTokens.delete(String(args[0]));
        if (!token || token.id !== id || token.expires < Date.now()) return socket.send({ event: "jwt error", args: ["This console's ticket isn't good (any more)."] });
        allowed = true;
        bot.watchers.add(socket);
        socket.send({ event: "auth success", args: [] });
        return socket.send({ event: "status", args: [bot.state] });
      }
      if (!allowed) return;
      if (message.event === "send logs") for (const line of bot.lines) socket.send({ event: "console output", args: [line] });
      if (message.event === "send command" && bot.child) bot.child.stdin.write(`${String(args[0] || "")}\n`);
      if (message.event === "set state") powerBot(bot, String(args[0]));
    };
    socket.onClose = () => {
      sockets.delete(socket);
      count(address, -1);
      bot.watchers.delete(socket);
    };
  }

  server.on("upgrade", (req, raw) => {
    const url = new URL(req.url, "http://host");
    const key = req.headers["sec-websocket-key"];
    const address = addressOf(req);
    const consoleOf = /^\/bot-console\/([^/]+)$/.exec(url.pathname);
    if (mode === "full" && consoleOf && PROJECT.test(consoleOf[1]) && key && (req.headers.upgrade || "").toLowerCase() === "websocket") {
      if ((perAddress.get(address) || 0) >= LIMITS.connectionsPerAddress) return raw.end("HTTP/1.1 429 Too Many Requests\r\nConnection: close\r\n\r\n");
      return botConsole(raw, key, address, consoleOf[1]);
    }
    if (url.pathname !== "/ws" || !key || (req.headers.upgrade || "").toLowerCase() !== "websocket") {
      raw.end("HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n");
      return;
    }
    if ((perAddress.get(address) || 0) >= LIMITS.connectionsPerAddress) {
      raw.end("HTTP/1.1 429 Too Many Requests\r\nConnection: close\r\n\r\n");
      return;
    }
    const accept = crypto.createHash("sha1").update(key + GUID).digest("base64");
    raw.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    const socket = new Socket(raw, address);
    sockets.add(socket);
    count(address, 1);
    socket.send({ t: "status", ...status() });
    socket.onMessage = (text) => {
      if (text.length > LIMITS.message) return socket.close(1009, "too big");
      let message;
      try {
        message = JSON.parse(text);
      } catch {
        return socket.send({ t: "error", why: "That isn't JSON." });
      }
      if (!message || typeof message.t !== "string") return socket.send({ t: "error", why: "A message needs a t." });
      try {
        handle(socket, message);
      } catch (e) {
        socket.send({ t: "error", re: message.t, why: "The host couldn't do that." });
        log(`Error in ${message.t}: ${e && e.message}`);
      }
    };
    socket.onClose = () => {
      sockets.delete(socket);
      count(address, -1);
      leave(socket);
    };
  });

  function leave(socket) {
    if (!socket.room) return;
    const room = rooms.get(socket.room);
    socket.room = null;
    if (!room) return;
    if (room.host === socket) {
      // The session is the host's: without them it is over.
      for (const guest of room.guests.values()) {
        guest.room = null;
        guest.send({ t: "closed" });
      }
      rooms.delete(room.id);
    } else {
      room.guests.delete(socket.id);
      room.host.send({ t: "peer", id: socket.id, on: false });
    }
  }

  function handle(socket, m) {
    const refuse = (why) => socket.send({ t: "error", re: m.t, why });
    switch (m.t) {
      case "ping":
        return socket.send({ t: "pong" });

      // ---- live sessions: passed on, never read, never kept ----
      case "open": {
        if (socket.room) return refuse("You are in a session already.");
        const mine = [...rooms.values()].filter((r) => r.host.address === socket.address).length;
        if (mine >= LIMITS.roomsPerAddress) return refuse("Too many sessions from this address.");
        const id = random(12);
        rooms.set(id, { id, host: socket, guests: new Map() });
        socket.room = id;
        socket.id = "host";
        return socket.send({ t: "opened", room: id });
      }
      case "join": {
        if (socket.room) return refuse("You are in a session already.");
        const room = rooms.get(String(m.room || ""));
        if (!room) return refuse("That session isn't open (any more).");
        if (room.guests.size >= LIMITS.guests) return refuse("That session is full.");
        socket.room = room.id;
        socket.id = random(6);
        room.guests.set(socket.id, socket);
        socket.send({ t: "joined", you: socket.id });
        return room.host.send({ t: "peer", id: socket.id, on: true });
      }
      case "to": {
        const room = rooms.get(socket.room || "");
        if (!room) return refuse("You are in no session.");
        if (room.host !== socket) return room.host.send({ t: "from", id: socket.id, data: m.data });
        // From the host: to one guest, or to all of them.
        if (m.id != null) {
          const guest = room.guests.get(String(m.id));
          return guest ? guest.send({ t: "from", id: "host", data: m.data }) : undefined;
        }
        for (const guest of room.guests.values()) guest.send({ t: "from", id: "host", data: m.data });
        return;
      }
      case "kick": {
        const room = rooms.get(socket.room || "");
        if (!room || room.host !== socket) return refuse("Only the session's host can do that.");
        const guest = room.guests.get(String(m.id));
        if (guest) {
          guest.send({ t: "closed" });
          leave(guest);
        }
        return;
      }
      case "leave":
        return leave(socket);
    }

    if (mode !== "full") return refuse("This host only passes live sessions on.");

    // ---- pairing: how a device gets in ----
    if (m.t === "pair") {
      const device = store.pair(m.code, m.name, m.key);
      if (!device) return refuse("That code isn't right, or it ran out.");
      setupCode = null;
      socket.device = { id: device.id, name: String(m.name || "Device") };
      return socket.send({ t: "paired", device: device.id, token: device.token });
    }
    if (m.t === "hello") {
      const device = store.device(m.token);
      if (!device) return refuse("This device isn't paired with the host (any more).");
      socket.device = device;
      return socket.send({ t: "welcome", device: device.id, name: device.name });
    }
    if (!socket.device) return refuse("Pair this device first.");

    switch (m.t) {
      case "invite":
        return socket.send({ t: "invited", code: store.invite(), minutes: LIMITS.codeMinutes });
      case "devices":
        return socket.send({ t: "devices", devices: store.devices(), you: socket.device.id });
      case "device.remove": {
        const removed = store.remove(String(m.id));
        for (const other of sockets) if (other.device && other.device.id === m.id) other.close(1008, "removed");
        return socket.send({ t: "device.removed", id: m.id, ok: removed });
      }
      // ---- the vault: secrets, encrypted by the devices ----
      case "vault.get": {
        if (!PROJECT.test(String(m.project))) return refuse("That isn't a project's id.");
        const row = store.vault(m.project);
        return socket.send({ t: "vault", project: m.project, blob: row ? row.blob : null, rev: row ? row.rev : 0 });
      }
      case "vault.put": {
        if (!PROJECT.test(String(m.project))) return refuse("That isn't a project's id.");
        if (typeof m.blob !== "string" || m.blob.length > LIMITS.vaultBlob) return refuse("That is too much for the vault.");
        const rev = store.putVault(m.project, m.blob, Number(m.rev) || 0);
        if (rev == null) return refuse("Another device stored newer secrets: get them first.");
        return socket.send({ t: "vault.kept", project: m.project, rev });
      }
      case "key.put": {
        if (typeof m.blob !== "string" || m.blob.length > LIMITS.keyBlob) return refuse("That is too much for a key.");
        if (!store.putKey(String(m.for), socket.device.id, m.blob)) return refuse("That device isn't paired, or too many keys wait for it.");
        for (const other of sockets) if (other.device && other.device.id === m.for) other.send({ t: "keys", keys: store.takeKeys(m.for) });
        return socket.send({ t: "key.kept", for: m.for });
      }
      case "key.get":
        return socket.send({ t: "keys", keys: store.takeKeys(socket.device.id) });
      case "projects": {
        const names = fs.readdirSync(projects).filter((n) => n.endsWith(".git")).map((n) => n.slice(0, -4));
        return socket.send({ t: "projects", projects: names });
      }
      default:
        return refuse("The host doesn't know that message.");
    }
  }

  // A sign of life both ways: proxies cut connections that say nothing.
  const beat = setInterval(() => {
    const now = Date.now();
    for (const socket of sockets) {
      if (now - socket.heard > LIMITS.quietSeconds * 1000) socket.close(1001, "quiet");
      else socket.ping();
    }
    for (const [token, ticket] of consoleTokens) if (ticket.expires < now) consoleTokens.delete(token);
    for (const bot of bots.values()) {
      if (!bot.child || !bot.watchers.size) continue;
      const stats = JSON.stringify({ memory_bytes: botMemory(bot), memory_limit_bytes: 0, cpu_absolute: 0 });
      for (const watcher of bot.watchers) watcher.send({ event: "stats", args: [stats] });
    }
  }, options.beatMs || 30_000);
  beat.unref();

  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(options.port, options.address || "0.0.0.0", () => {
      const port = server.address().port;
      log(`XC Circuit Host ${VERSION} in ${mode} mode on port ${port}`);
      resolve({
        port,
        setupCode: () => setupCode,
        close: () =>
          new Promise((done) => {
            clearInterval(beat);
            // The bots end with the host; which were running is noted, so
            // they run again when it is back.
            for (const bot of bots.values()) {
              if (bot.installing) bot.installing.kill();
              if (bot.child) bot.child.kill("SIGTERM");
              bot.child = null;
            }
            for (const socket of sockets) socket.close(1001, "closing");
            server.close(() => {
              if (store) store.close();
              done();
            });
            server.closeAllConnections();
          }),
      });
    });
  });
}

module.exports = { start, settings, LIMITS, PROTOCOL, VERSION };

if (require.main === module) {
  let options;
  try {
    options = settings();
  } catch (e) {
    console.error(e.message);
    process.exit(78);
  }
  start(options)
    .then((host) => {
      const stop = () => host.close().then(() => process.exit(0));
      process.on("SIGTERM", stop);
      process.on("SIGINT", stop);
    })
    .catch((e) => {
      console.error(e.message);
      process.exit(78);
    });
}
