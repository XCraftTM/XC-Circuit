// Docker: lists containers, starts, stops or restarts one, or reads its
// latest log lines, through Docker's own API (its socket on this computer,
// or tcp://host:port from Bot › Settings › Server tools).

const ACTIONS = { list: "List containers", start: "Start", stop: "Stop", restart: "Restart", logs: "Latest log lines" };

function allowed(ctx) {
  if (ctx.runtime.setting("serverToolsAllowed") !== true) throw new Error("Server tools are off: allow them in Bot › Settings › Server tools");
}

/** Asks Docker: { status, body } (body as bytes). */
function docker(where, method, path) {
  const http = require("http");
  const s = String(where || "").trim();
  const target = /^tcp:\/\//i.test(s)
    ? (() => {
        const u = new URL(s.replace(/^tcp:/i, "http:"));
        return { host: u.hostname, port: Number(u.port) || 2375 };
      })()
    : { socketPath: s.replace(/^unix:\/\//i, "") || (process.platform === "win32" ? "//./pipe/docker_engine" : "/var/run/docker.sock") };
  return new Promise((resolve, reject) => {
    const req = http.request({ ...target, method, path, timeout: 15000, headers: { Host: "docker" } }, (res) => {
      const chunks = [];
      res.on("data", (c) => chunks.push(c));
      res.on("end", () => resolve({ status: res.statusCode, body: Buffer.concat(chunks) }));
    });
    req.on("timeout", () => req.destroy(new Error("Docker didn't answer in 15 s")));
    req.on("error", (e) =>
      reject(new Error(e.code === "ENOENT" || e.code === "ECONNREFUSED" ? "Docker isn't running here (or the bot may not use it: add its user to the docker group)" : e.code === "EACCES" ? "the bot may not use Docker: add its user to the docker group" : e.message)),
    );
    req.end();
  });
}

/** Log lines from Docker's stream: frames of 8-byte headers, unless the container has a terminal. */
function logText(buf) {
  let out = "";
  let at = 0;
  while (at + 8 <= buf.length && buf[at] <= 2 && buf[at + 1] === 0 && buf[at + 2] === 0 && buf[at + 3] === 0) {
    const size = buf.readUInt32BE(at + 4);
    out += buf.subarray(at + 8, at + 8 + size).toString("utf8");
    at += 8 + size;
  }
  return at === 0 ? buf.toString("utf8") : out;
}

module.exports = {
  id: "server.docker",
  kind: "action",
  info: {
    name: "Docker",
    description: "Lists Docker containers, starts, stops or restarts one, or reads its latest log lines.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "servertools",
    aliases: ["Containers", "Restart container", "Docker logs"],
    tags: ["docker", "container", "restart", "logs", "server"],
  },
  look: {
    icon: "terminal",
  },
  fields: {
    action: { type: "select", pin: false, default: "list", choices: ACTIONS, description: "What to do." },
    container: { type: "text", default: "", required: true, show: { action: ["start", "stop", "restart", "logs"] }, description: "The container's name (or id)." },
    lines: { type: "number", pin: false, default: 20, min: 1, max: 1000, show: { action: ["logs"] }, description: "How many of the latest lines." },
  },
  outputs: {
    containers: { type: "list<object>", description: "List: each with name, id, image, state (running, exited …) and status (Up 3 hours …)." },
    logs: { type: "text", description: "Latest log lines: the text." },
    ok: { type: "boolean", description: "Whether it worked (starting a running container counts)." },
  },
  async run({ fields, ctx }) {
    allowed(ctx);
    const where = ctx.runtime.setting("dockerHost");
    const name = encodeURIComponent(String(fields.container || "").trim().replace(/^\//, ""));
    const fail = (r) => {
      let msg = r.body.toString("utf8");
      try {
        msg = JSON.parse(msg).message;
      } catch {}
      throw new Error(`Docker: ${msg || r.status}`);
    };
    switch (fields.action) {
      case "start":
      case "stop":
      case "restart": {
        const r = await docker(where, "POST", `/containers/${name}/${fields.action}`);
        // 304: already started or stopped.
        if (r.status !== 204 && r.status !== 304) fail(r);
        return { containers: [], logs: "", ok: true };
      }
      case "logs": {
        const r = await docker(where, "GET", `/containers/${name}/logs?stdout=1&stderr=1&tail=${Math.max(1, Number(fields.lines) || 20)}`);
        if (r.status !== 200) fail(r);
        return { containers: [], logs: logText(r.body).trimEnd(), ok: true };
      }
      default: {
        const r = await docker(where, "GET", "/containers/json?all=1");
        if (r.status !== 200) fail(r);
        const containers = JSON.parse(r.body.toString("utf8")).map((c) => ({ name: String(c.Names?.[0] || "").replace(/^\//, ""), id: String(c.Id).slice(0, 12), image: c.Image, state: c.State, status: c.Status }));
        return { containers, logs: "", ok: true };
      }
    }
  },
};
