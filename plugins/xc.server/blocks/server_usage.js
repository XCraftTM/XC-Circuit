// Server Usage: CPU, memory, disk and uptime, of the computer the bot runs
// on, or of the SSH server of Bot › Settings › Server tools.

const WHERE = { here: "The computer the bot runs on", ssh: "The SSH server" };

function allowed(ctx) {
  if (ctx.runtime.setting("serverToolsAllowed") !== true) throw new Error("Server tools are off: allow them in Bot › Settings › Server tools");
}

/**
 * Runs `cmd` over SSH. The server's key is remembered the first time and
 * checked after that, so a different machine in its place is refused.
 */
async function sshRun(ctx, cmd, timeoutMs) {
  const { Client } = ctx.require("ssh2");
  const host = String(ctx.runtime.setting("sshHost") || "").trim();
  if (!host) throw new Error("No SSH server yet: Bot › Settings › Server tools › SSH");
  const port = Number(ctx.runtime.setting("sshPort")) || 22;
  const key = ctx.runtime.secret("sshKey");
  const password = ctx.runtime.secret("sshPassword");
  const known = ctx.store.ns("xc.server");
  const remembered = await known.get(`hostkey:${host}:${port}`);
  let seen = null;
  return new Promise((resolve, reject) => {
    const c = new Client();
    let out = "";
    let err = "";
    const done = (fn, v) => {
      clearTimeout(timer);
      c.end();
      fn(v);
    };
    const timer = setTimeout(() => done(reject, new Error(`${host} didn't finish in ${Math.round(timeoutMs / 1000)} s`)), timeoutMs);
    c.on("ready", async () => {
      if (!remembered && seen) await known.set(`hostkey:${host}:${port}`, seen);
      c.exec(cmd, (e, stream) => {
        if (e) return done(reject, e);
        stream.on("data", (d) => (out = (out + d).slice(-1_000_000)));
        stream.stderr.on("data", (d) => (err = (err + d).slice(-1_000_000)));
        stream.on("close", (code) => done(resolve, { out, err, code: code ?? 0 }));
      });
    });
    c.on("error", (e) => {
      const text =
        e.level === "client-authentication" ? "the user name, password or key is wrong" : e.code === "ECONNREFUSED" ? `nothing answers on port ${port}` : e.code === "ENOTFOUND" ? "the host wasn't found" : e.message;
      done(reject, new Error(`SSH to ${host}: ${text}`));
    });
    c.connect({
      host,
      port,
      username: String(ctx.runtime.setting("sshUser") || "").trim() || undefined,
      password: key ? undefined : password,
      privateKey: key || undefined,
      passphrase: key ? password : undefined,
      readyTimeout: 15000,
      hostHash: "sha256",
      hostVerifier: (hash) => {
        seen = hash;
        if (remembered && remembered !== hash) {
          ctx.log.error(`SSH to ${host}: the server's key isn't the one it had before. If you replaced the server, delete the stored value hostkey:${host}:${port} (ns xc.server).`);
          return false;
        }
        return true;
      },
    });
  });
}

const pct = (part, whole) => (whole > 0 ? Math.round((part / whole) * 1000) / 10 : 0);
const gb = (b) => `${(b / 1024 ** 3).toFixed(1)} GB`;

function summary(u) {
  const days = Math.floor(u.uptime / 86400);
  const hours = Math.floor((u.uptime % 86400) / 3600);
  return `CPU ${u.cpu} % · RAM ${u.memory} % (${gb(u.memoryUsed)} of ${gb(u.memoryTotal)}) · Disk ${u.disk} % (${gb(u.diskUsed)} of ${gb(u.diskTotal)}) · up ${days} d ${hours} h`;
}

async function here(dataDir) {
  const os = require("os");
  const fs = require("fs");
  const times = () => os.cpus().map((c) => c.times).reduce((a, t) => ({ idle: a.idle + t.idle, all: a.all + t.user + t.nice + t.sys + t.idle + t.irq }), { idle: 0, all: 0 });
  const a = times();
  await new Promise((r) => setTimeout(r, 500));
  const b = times();
  const busy = b.all - a.all > 0 ? 1 - (b.idle - a.idle) / (b.all - a.all) : 0;
  // The disk the data folder is on (or will be: the nearest folder that's there).
  let dir = dataDir;
  while (!fs.existsSync(dir) && require("path").dirname(dir) !== dir) dir = require("path").dirname(dir);
  const disk = fs.statfsSync(dir);
  const diskTotal = disk.blocks * disk.bsize;
  const diskFree = disk.bavail * disk.bsize;
  const memoryTotal = os.totalmem();
  const memoryUsed = memoryTotal - os.freemem();
  return { cpu: Math.round(busy * 1000) / 10, memoryUsed, memoryTotal, diskUsed: diskTotal - diskFree, diskTotal, uptime: Math.round(os.uptime()) };
}

/** The same over SSH, from /proc, free and df (Linux). */
const SSH_COMMAND = "head -1 /proc/stat; sleep 0.5; head -1 /proc/stat; free -b | grep -i '^mem'; df -B1 -P / | tail -1; cat /proc/uptime";

function parseSsh(out) {
  const lines = out.trim().split("\n");
  const cpu = (l) => {
    const n = l.trim().split(/\s+/).slice(1).map(Number);
    return { idle: n[3] + (n[4] || 0), all: n.reduce((s, x) => s + (x || 0), 0) };
  };
  const [a, b] = [cpu(lines[0]), cpu(lines[1])];
  const busy = b.all - a.all > 0 ? 1 - (b.idle - a.idle) / (b.all - a.all) : 0;
  const mem = lines[2].trim().split(/\s+/).map(Number);
  // free: total used free shared buff/cache available; "used" by what's left available.
  const memoryTotal = mem[1];
  const memoryUsed = Number.isFinite(mem[6]) ? memoryTotal - mem[6] : mem[2];
  const df = lines[3].trim().split(/\s+/);
  return { cpu: Math.round(busy * 1000) / 10, memoryUsed, memoryTotal, diskUsed: Number(df[2]), diskTotal: Number(df[1]), uptime: Math.round(Number(lines[4].split(/\s+/)[0])) };
}

module.exports = {
  id: "server.server_usage",
  kind: "action",
  info: {
    name: "Server Usage",
    description: "How busy a computer is: CPU, memory, disk and how long it's been up. The one the bot runs on, or the SSH server (Linux).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "servertools",
    aliases: ["CPU", "RAM", "Disk space", "Uptime", "Server stats", "System info"],
    tags: ["cpu", "ram", "memory", "disk", "uptime", "server", "stats", "load"],
  },
  look: {
    icon: "terminal",
  },
  requires: {
    dependencies: { ssh2: "^1.17.0" },
  },
  fields: {
    where: { type: "select", pin: false, default: "here", choices: WHERE, description: "Which computer. The SSH server needs Server tools allowed in Bot › Settings." },
  },
  outputs: {
    cpu: { type: "number", description: "How busy the processor is, in %." },
    memory: { type: "number", description: "Memory in use, in %." },
    disk: { type: "number", description: "Disk space in use, in %." },
    memoryUsed: { type: "number", description: "Memory in use, in bytes." },
    memoryTotal: { type: "number", description: "All memory, in bytes." },
    diskUsed: { type: "number", description: "Disk space in use, in bytes." },
    diskTotal: { type: "number", description: "All disk space, in bytes." },
    uptime: { type: "number", description: "Seconds since it started." },
    text: { type: "text", description: "All of it in one line: CPU 12 % · RAM 41 % (3.2 GB of 7.8 GB) · …" },
  },
  async run({ fields, ctx }) {
    let u;
    if (fields.where === "ssh") {
      allowed(ctx);
      const r = await sshRun(ctx, SSH_COMMAND, 20000);
      if (r.code !== 0) throw new Error(`The server didn't answer the usage commands (is it Linux?): ${r.err.trim()}`);
      u = parseSsh(r.out);
    } else {
      u = await here(ctx.runtime.dataDir);
    }
    const out = { ...u, memory: pct(u.memoryUsed, u.memoryTotal), disk: pct(u.diskUsed, u.diskTotal) };
    return { ...out, text: summary(out) };
  },

  // For tests.
  parseSsh,
  SSH_COMMAND,
};
