// Minecraft Server: whether a server is up, who's on and its message of the
// day. Java Edition answers the Server List Ping (as the game's server list
// asks), Bedrock the RakNet "unconnected ping".

const EDITIONS = { java: "Java Edition", bedrock: "Bedrock Edition" };

function varint(n) {
  const out = [];
  let v = n >>> 0;
  do {
    let b = v & 0x7f;
    v >>>= 7;
    if (v) b |= 0x80;
    out.push(b);
  } while (v);
  return Buffer.from(out);
}

/** A VarInt at `at` in `buf`: [value, bytes], or null when it isn't all there yet. */
function readVarint(buf, at) {
  let v = 0;
  for (let i = 0; i < 5; i++) {
    if (at + i >= buf.length) return null;
    const b = buf[at + i];
    v |= (b & 0x7f) << (7 * i);
    if (!(b & 0x80)) return [v, i + 1];
  }
  throw new Error("the server's answer isn't Minecraft's");
}

const packet = (...parts) => {
  const body = Buffer.concat(parts);
  return Buffer.concat([varint(body.length), body]);
};
const mcString = (s) => {
  const b = Buffer.from(s, "utf8");
  return Buffer.concat([varint(b.length), b]);
};

/** A chat component (or text) as plain text, without § colour codes. */
function chatText(c) {
  if (c == null) return "";
  if (typeof c === "string") return c.replace(/§./g, "");
  return (chatText(c.text) + (c.extra || []).map(chatText).join("")).replace(/§./g, "");
}

async function java(host, port, timeout) {
  const net = require("net");
  // A name without a port may have an SRV record pointing elsewhere (as the game looks for).
  if (!port) {
    try {
      const [srv] = await require("dns").promises.resolveSrv(`_minecraft._tcp.${host}`);
      if (srv) [host, port] = [srv.name, srv.port];
    } catch {}
  }
  port = port || 25565;
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const socket = net.connect({ host, port });
    let buf = Buffer.alloc(0);
    const fail = (e) => {
      socket.destroy();
      reject(e);
    };
    socket.setTimeout(timeout, () => fail(new Error(`${host}:${port} didn't answer in ${timeout / 1000} s`)));
    socket.on("error", (e) => fail(new Error(e.code === "ECONNREFUSED" ? `nothing answers at ${host}:${port}` : e.code === "ENOTFOUND" ? `${host} wasn't found` : e.message)));
    socket.on("connect", () => {
      const p = Buffer.alloc(2);
      p.writeUInt16BE(port);
      // Handshake (protocol -1: "any", next state 1: status), then the status request.
      socket.write(packet(varint(0), varint(-1), mcString(host), p, varint(1)));
      socket.write(packet(varint(0)));
    });
    socket.on("data", (d) => {
      buf = Buffer.concat([buf, d]);
      try {
        const len = readVarint(buf, 0);
        if (!len || buf.length < len[1] + len[0]) return;
        let at = len[1];
        const id = readVarint(buf, at);
        at += id[1];
        const strLen = readVarint(buf, at);
        at += strLen[1];
        const status = JSON.parse(buf.subarray(at, at + strLen[0]).toString("utf8"));
        socket.destroy();
        resolve({
          online: true,
          players: status.players?.online ?? 0,
          max: status.players?.max ?? 0,
          names: (status.players?.sample || []).map((p) => p.name).filter((n) => n && !/§/.test(n)),
          version: chatText(status.version?.name),
          motd: chatText(status.description).trim(),
          icon: status.favicon || "",
          ms: Date.now() - started,
        });
      } catch (e) {
        fail(e);
      }
    });
  });
}

/** RakNet's "offline message" marker. */
const MAGIC = "00ffff00fefefefefdfdfdfd12345678";

async function bedrock(host, port, timeout) {
  const dgram = require("dgram");
  port = port || 19132;
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const socket = dgram.createSocket(require("net").isIPv6(host) ? "udp6" : "udp4");
    const timer = setTimeout(() => {
      socket.close();
      reject(new Error(`${host}:${port} didn't answer in ${timeout / 1000} s`));
    }, timeout);
    socket.on("error", (e) => {
      clearTimeout(timer);
      socket.close();
      reject(new Error(e.code === "ENOTFOUND" ? `${host} wasn't found` : e.message));
    });
    socket.on("message", (msg) => {
      if (msg[0] !== 0x1c) return;
      clearTimeout(timer);
      socket.close();
      // 1c, time (8), server guid (8), magic (16), length (2), MCPE;motd;protocol;version;players;max;id;motd 2;mode;…
      const text = msg.subarray(35, 35 + msg.readUInt16BE(33)).toString("utf8");
      const f = text.split(";");
      resolve({ online: true, players: Number(f[4]) || 0, max: Number(f[5]) || 0, names: [], version: f[3] || "", motd: [f[1], f[7]].filter(Boolean).join("\n").replace(/§./g, ""), icon: "", ms: Date.now() - started });
    });
    const ping = Buffer.alloc(33);
    ping[0] = 0x01;
    ping.writeBigInt64BE(BigInt(Date.now()), 1);
    Buffer.from(MAGIC, "hex").copy(ping, 9);
    ping.writeBigInt64BE(2n, 25);
    socket.send(ping, port, host);
  });
}

module.exports = {
  id: "server.minecraft_status",
  kind: "action",
  info: {
    name: "Minecraft Server",
    description: "Whether a Minecraft server (Java or Bedrock) is up, how many play on it (and who, for Java), its version and message of the day.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "servertools",
    aliases: ["Minecraft status", "MC server", "Players online", "Server ping"],
    tags: ["minecraft", "mc", "server", "status", "players", "online", "java", "bedrock", "ping"],
  },
  look: {
    icon: "terminal",
  },
  fields: {
    edition: { type: "select", pin: false, default: "java", choices: EDITIONS, description: "Which Minecraft." },
    address: { type: "text", default: "play.example.com", required: true, description: "The server's address, as players type it: play.example.com or 203.0.113.5:25566." },
    seconds: { type: "number", pin: false, default: 5, min: 1, max: 60, advanced: true, label: "Timeout (s)", description: "How long to wait for an answer." },
  },
  outputs: {
    online: { type: "boolean", description: "Whether it answered." },
    players: { type: "number", description: "How many are on." },
    max: { type: "number", description: "How many may be." },
    names: { type: "list<text>", description: "Some of the players' names (Java, when the server shares them)." },
    version: { type: "text", description: "Its version, such as 1.21.4 (or a server software's name)." },
    motd: { type: "text", description: "Its message of the day, without colour codes." },
    icon: { type: "text", description: "Its icon, as a data: address (Java), or empty." },
    ms: { type: "number", description: "How long it took to answer, in ms." },
    error: { type: "text", description: "Why it's offline, when it is." },
  },
  async run({ fields }) {
    const raw = String(fields.address || "").trim().replace(/^[a-z]+:\/\//i, "");
    const m = /^\[?([^\]]+?)\]?(?::(\d+))?$/.exec(raw);
    if (!m) throw new Error(`"${fields.address}" isn't a server address`);
    const [host, port] = [m[1], m[2] ? Number(m[2]) : 0];
    const timeout = (Number(fields.seconds) || 5) * 1000;
    try {
      return { ...(await (fields.edition === "bedrock" ? bedrock : java)(host, port, timeout)), error: "" };
    } catch (e) {
      // Offline is an answer, not an error: the flow goes on.
      return { online: false, players: 0, max: 0, names: [], version: "", motd: "", icon: "", ms: 0, error: e.message };
    }
  },
};
