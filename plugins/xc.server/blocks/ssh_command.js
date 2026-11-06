// SSH Command: runs a command on the server of Bot › Settings › Server tools.
// Each ${…} in the command is put in quotes for the shell, so what people
// type (a name, a reason) can't run commands of its own.

function allowed(ctx) {
  if (ctx.runtime.setting("serverToolsAllowed") !== true) throw new Error("Server tools are off: allow them in Bot › Settings › Server tools");
}

/** Servers given on a block (from a command's options, say) need their own switch. */
function givenAllowed(ctx) {
  if (ctx.runtime.setting("serverGivenAllowed") !== true) throw new Error("Servers given on a block are off: allow them in Bot › Settings › Server tools › Safety");
}

const SERVER = { settings: "From Bot Settings", given: "Given here" };

/** A value as one word for a POSIX shell: in single quotes. */
const quote = (v) => `'${String(v ?? "").replace(/'/g, "'\\''")}'`;

/** The command with each ${…} quoted. */
function command(parts) {
  if (typeof parts === "string") return parts;
  let out = parts.strings[0];
  parts.values.forEach((v, i) => (out += quote(v && typeof v === "object" && typeof v.id === "string" ? v.id : v) + parts.strings[i + 1]));
  return out;
}

/** The SSH login: the one of Bot › Settings › Server tools, or the one the block gives. */
function sshLogin(ctx, fields, given) {
  const text = (v) => (v === undefined || v === null ? "" : String(v));
  if (given) {
    givenAllowed(ctx);
    const host = text(fields.host).trim();
    if (!host) throw new Error("The block gives no SSH host.");
    return { host, port: Number(fields.port) || 22, user: text(fields.user).trim(), password: text(fields.password) || undefined, key: text(fields.key) || undefined };
  }
  const host = text(ctx.runtime.setting("sshHost")).trim();
  if (!host) throw new Error("No SSH server yet: Bot › Settings › Server tools › SSH");
  return {
    host,
    port: Number(ctx.runtime.setting("sshPort")) || 22,
    user: text(ctx.runtime.setting("sshUser")).trim(),
    password: ctx.runtime.secret("sshPassword") || undefined,
    key: ctx.runtime.secret("sshKey") || undefined,
  };
}

/**
 * Runs `cmd` over SSH with `login` (from sshLogin). The server's key is
 * remembered the first time and checked after that, so a different machine
 * in its place is refused.
 */
async function sshRun(ctx, login, cmd, timeoutMs) {
  const { Client } = ctx.require("ssh2");
  const { host, port, key, password } = login;
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
      username: login.user || undefined,
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

module.exports = {
  id: "server.ssh_command",
  kind: "action",
  info: {
    name: "SSH Command",
    description: "Runs a command on the SSH server of Bot › Settings › Server tools, or one given on the block, and gives what it printed. ${…} values are quoted, so they can't run commands of their own.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "servertools",
    aliases: ["SSH", "Shell", "Terminal", "Run on server", "Linux command"],
    tags: ["ssh", "shell", "server", "command", "linux", "terminal", "vps"],
  },
  look: {
    icon: "terminal",
  },
  requires: {
    dependencies: { ssh2: "^1.17.0" },
  },
  fields: {
    server: { type: "select", pin: false, default: "settings", choices: SERVER, description: "Bot › Settings › Server tools › SSH, or a server given here (from a command's options, say). Given here needs its own switch in Bot › Settings › Server tools." },
    host: { type: "text", default: "", show: { server: ["given"] }, placeholder: "server.example.com", description: "The SSH server's address." },
    port: { type: "number", default: 22, min: 1, max: 65535, show: { server: ["given"] } },
    user: { type: "text", default: "", show: { server: ["given"] }, placeholder: "bot" },
    password: { type: "text", pin: "only", show: { server: ["given"] }, description: "Connect it from a command's option or a form (only the bot sees those): typed into a block, it would be saved in the project file. With a key, the key's passphrase." },
    key: { type: "text", pin: "only", show: { server: ["given"] }, label: "Private key", description: "The whole private key, instead of a password. Connected, like the password." },
    command: {
      type: "textarea",
      pin: false,
      templateParts: true,
      required: true,
      default: "systemctl status ${service} --no-pager",
      description: "A command for the server's shell. ${…} are values, put in quotes for you: whitelist add ${name} is safe even when the name has ; or $ in it.",
    },
    seconds: { type: "number", pin: false, default: 30, min: 1, max: 3600, advanced: true, label: "Timeout (s)", description: "Give up after this long." },
    failOnError: { type: "boolean", pin: false, default: false, advanced: true, label: "Stop when it fails", description: "When the command ends with a code other than 0, stop with an error instead of going on." },
  },
  outputs: {
    output: { type: "text", description: "What it printed." },
    errors: { type: "text", description: "What it printed as errors (stderr)." },
    code: { type: "number", description: "Its exit code: 0 when it worked." },
  },
  async run({ fields, ctx }) {
    allowed(ctx);
    const cmd = command(fields.command);
    const r = await sshRun(ctx, sshLogin(ctx, fields, fields.server === "given"), cmd, (Number(fields.seconds) || 30) * 1000);
    if (fields.failOnError && r.code !== 0) throw new Error(`The command ended with code ${r.code}: ${r.err.trim().split("\n").pop() || r.out.trim().split("\n").pop() || ""}`);
    return { output: r.out.trimEnd(), errors: r.err.trimEnd(), code: r.code };
  },

  // For tests.
  sshRun,
  sshLogin,
  command,
};
