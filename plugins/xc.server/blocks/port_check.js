module.exports = {
  id: "server.port_check",
  kind: "action",
  info: {
    name: "Port Check",
    description: "Whether a server answers on a port (a website, a game server, a database), and how fast.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "servertools",
    aliases: ["Is it up", "Ping", "Server online", "Uptime check"],
    tags: ["ping", "port", "online", "up", "down", "server", "check", "tcp"],
  },
  look: {
    icon: "terminal",
  },
  fields: {
    host: { type: "text", default: "example.com", required: true, description: "The server's name or address." },
    port: { type: "number", default: 443, min: 1, max: 65535, description: "The port: 443 for websites, 25565 for Minecraft, 22 for SSH …" },
    seconds: { type: "number", pin: false, default: 5, min: 1, max: 60, advanced: true, label: "Timeout (s)" },
  },
  outputs: {
    open: { type: "boolean", description: "Whether it answered." },
    ms: { type: "number", description: "How long connecting took, in ms." },
    error: { type: "text", description: "Why not, when it didn't: refused, not found, no answer." },
  },
  run({ fields }) {
    const net = require("net");
    const host = String(fields.host || "").trim().replace(/^[a-z]+:\/\//i, "").split(/[/:]/)[0];
    const started = Date.now();
    return new Promise((resolve) => {
      const socket = net.connect({ host, port: Number(fields.port) || 443 });
      const done = (open, error = "") => {
        socket.destroy();
        resolve({ open, ms: open ? Date.now() - started : 0, error });
      };
      socket.setTimeout((Number(fields.seconds) || 5) * 1000, () => done(false, "no answer"));
      socket.on("connect", () => done(true));
      socket.on("error", (e) => done(false, e.code === "ECONNREFUSED" ? "refused" : e.code === "ENOTFOUND" ? "not found" : e.message));
    });
  },
};
