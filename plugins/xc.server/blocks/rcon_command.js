// RCON Command: sends a command to a game server's console (Minecraft,
// Rust, ARK, CS2 … everything with Source RCON) and gives its answer.

function allowed(ctx) {
  if (ctx.runtime.setting("serverToolsAllowed") !== true) throw new Error("Server tools are off: allow them in Bot › Settings › Server tools");
}

const AUTH = 3;
const COMMAND = 2;

function frame(id, type, body) {
  const b = Buffer.from(body, "utf8");
  const out = Buffer.alloc(14 + b.length);
  out.writeInt32LE(10 + b.length, 0);
  out.writeInt32LE(id, 4);
  out.writeInt32LE(type, 8);
  b.copy(out, 12);
  return out;
}

/** Logs in and runs `command`; resolves to its answer. */
function rcon(host, port, password, command, timeout) {
  const net = require("net");
  return new Promise((resolve, reject) => {
    const socket = net.connect({ host, port });
    let buf = Buffer.alloc(0);
    let answer = "";
    let quiet = null;
    let loggedIn = false;
    const end = (fn, v) => {
      clearTimeout(quiet);
      socket.destroy();
      fn(v);
    };
    socket.setTimeout(timeout, () => (loggedIn && answer ? end(resolve, answer) : end(reject, new Error(`${host}:${port} didn't answer in ${timeout / 1000} s`))));
    socket.on("error", (e) => end(reject, new Error(e.code === "ECONNREFUSED" ? `nothing answers at ${host}:${port}: is RCON on?` : e.message)));
    socket.on("connect", () => socket.write(frame(1, AUTH, password)));
    socket.on("data", (d) => {
      buf = Buffer.concat([buf, d]);
      while (buf.length >= 4 && buf.length >= 4 + buf.readInt32LE(0)) {
        const size = buf.readInt32LE(0);
        const id = buf.readInt32LE(4);
        const type = buf.readInt32LE(8);
        const body = buf.subarray(12, 4 + size - 2).toString("utf8");
        buf = buf.subarray(4 + size);
        if (!loggedIn) {
          if (type !== AUTH - 1) continue; // some servers send an empty answer before the login's
          if (id === -1) return end(reject, new Error("the RCON password is wrong"));
          loggedIn = true;
          socket.write(frame(2, COMMAND, command));
          continue;
        }
        if (id !== 2) continue;
        answer += body;
        // Long answers come in several parts: done once nothing more comes for a moment.
        clearTimeout(quiet);
        quiet = setTimeout(() => end(resolve, answer), 150);
      }
    });
  });
}

module.exports = {
  id: "server.rcon_command",
  kind: "action",
  info: {
    name: "RCON Command",
    description: "Sends a command to a game server's console over RCON (Minecraft, Rust, ARK, CS2 …) and gives its answer, such as whitelist add ${name} or list.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "servertools",
    aliases: ["RCON", "Minecraft command", "Game server console", "Whitelist"],
    tags: ["rcon", "minecraft", "rust", "ark", "game", "server", "console", "command", "whitelist"],
  },
  look: {
    icon: "terminal",
  },
  fields: {
    command: { type: "text", default: "list", required: true, description: "The console command, without a /: list, whitelist add ${name}, say Hello." },
    address: { type: "text", default: "", advanced: true, description: "Another server than Bot › Settings › Server tools › RCON: host:port (the password there is used)." },
    seconds: { type: "number", pin: false, default: 10, min: 1, max: 120, advanced: true, label: "Timeout (s)" },
  },
  outputs: {
    answer: { type: "text", description: "What the server answered, without colour codes." },
  },
  async run({ fields, ctx }) {
    allowed(ctx);
    const other = String(fields.address || "").trim();
    const host = other ? other.replace(/:\d+$/, "") : String(ctx.runtime.setting("rconHost") || "").trim();
    const port = other && /:(\d+)$/.test(other) ? Number(/:(\d+)$/.exec(other)[1]) : Number(ctx.runtime.setting("rconPort")) || 25575;
    if (!host) throw new Error("No RCON server yet: Bot › Settings › Server tools › RCON");
    const password = ctx.runtime.secret("rconPassword") || "";
    const answer = await rcon(host, port, password, String(fields.command || "").replace(/^\//, ""), (Number(fields.seconds) || 10) * 1000);
    return { answer: answer.replace(/§./g, "").trim() };
  },
};
