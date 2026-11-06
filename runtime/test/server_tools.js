// The Server Tools blocks in a running bot, against small stand-in servers:
// RCON (login, a long answer in parts, a wrong password), Minecraft Java and
// Bedrock status, Docker's API, a port check and this computer's usage.
// With ssh2 in XC_TEST_DEPS also SSH: ${…} quoted for the shell, usage over
// SSH, and a server whose key changed refused. Without allowing server tools
// in Bot › Settings, the powerful blocks refuse.
// Usage: [XC_TEST_DEPS=<dir with node_modules/ssh2>] node runtime/test/server_tools.js
"use strict";

const fs = require("fs");
const os = require("os");
const net = require("net");
const http = require("http");
const dgram = require("dgram");
const path = require("path");
const Module = require("module");
const assert = require("assert");
const { spawn } = require("child_process");

const root = path.join(__dirname, "..", "..");
const blocks = {};
for (const folder of [path.join(root, "plugins", "xc.server", "blocks"), path.join(root, "plugins", "xc.core", "blocks", "events"), path.join(root, "plugins", "xc.core", "blocks", "debug")]) {
  for (const f of fs.readdirSync(folder)) blocks[require(path.join(folder, f)).id] = { source: fs.readFileSync(path.join(folder, f), "utf8"), file: path.join(folder, f) };
}
const deps = process.env.XC_TEST_DEPS ? path.resolve(process.env.XC_TEST_DEPS) : null;
let ssh2 = null;
try {
  ssh2 = deps && Module.createRequire(path.join(deps, "package.json"))("ssh2");
} catch {}

const listen = (server, ...args) => new Promise((r) => server.listen(0, "127.0.0.1", ...args, () => r(server.address().port)));

// ── RCON: password "pw"; "list" answers in two parts.
function rconServer(password = "pw") {
  return net.createServer((s) => {
    let buf = Buffer.alloc(0);
    const send = (id, type, body) => {
      const b = Buffer.from(body);
      const out = Buffer.alloc(14 + b.length);
      out.writeInt32LE(10 + b.length, 0);
      out.writeInt32LE(id, 4);
      out.writeInt32LE(type, 8);
      b.copy(out, 12);
      s.write(out);
    };
    s.on("data", (d) => {
      buf = Buffer.concat([buf, d]);
      while (buf.length >= 4 && buf.length >= 4 + buf.readInt32LE(0)) {
        const size = buf.readInt32LE(0);
        const [id, type] = [buf.readInt32LE(4), buf.readInt32LE(8)];
        const body = buf.subarray(12, 4 + size - 2).toString();
        buf = buf.subarray(4 + size);
        if (type === 3) {
          send(id, 0, ""); // an empty answer first, as some servers do
          send(body === password ? id : -1, 2, "");
        } else if (body === "list") {
          send(id, 0, "There are 2 of 10 players online: ");
          setTimeout(() => send(id, 0, "§aAlex§r, Bea"), 20);
        } else send(id, 0, `Unknown command ${body}`);
      }
    });
  });
}

// ── Minecraft Java: answers the status request with a JSON status.
function javaServer() {
  return net.createServer((s) => {
    s.once("data", () => {
      const json = Buffer.from(JSON.stringify({ version: { name: "1.21.4" }, players: { online: 3, max: 20, sample: [{ name: "Alex" }, { name: "Bea" }] }, description: { text: "§6Welcome", extra: [{ text: " home" }] }, favicon: "data:image/png;base64,AA" }));
      const vi = (n) => {
        const o = [];
        do {
          let b = n & 0x7f;
          n >>>= 7;
          if (n) b |= 0x80;
          o.push(b);
        } while (n);
        return Buffer.from(o);
      };
      const body = Buffer.concat([vi(0), vi(json.length), json]);
      setTimeout(() => s.write(Buffer.concat([vi(body.length), body])), 30);
    });
  });
}

// ── Docker's API: two containers, restart, framed logs.
function dockerServer() {
  return http.createServer((req, res) => {
    if (req.method === "GET" && req.url === "/containers/json?all=1") {
      res.end(JSON.stringify([{ Names: ["/web"], Id: "abcdef1234567890", Image: "nginx", State: "running", Status: "Up 2 hours" }, { Names: ["/db"], Id: "1234", Image: "mariadb", State: "exited", Status: "Exited (0)" }]));
    } else if (req.method === "POST" && req.url === "/containers/web/restart") {
      res.statusCode = 204;
      res.end();
    } else if (req.url.startsWith("/containers/web/logs")) {
      const f = (stream, text) => {
        const h = Buffer.alloc(8);
        h[0] = stream;
        h.writeUInt32BE(Buffer.byteLength(text), 4);
        return Buffer.concat([h, Buffer.from(text)]);
      };
      res.end(Buffer.concat([f(1, "started\n"), f(2, "warning: slow\n")]));
    } else {
      res.statusCode = 404;
      res.end(JSON.stringify({ message: "No such container: nope" }));
    }
  });
}

/** Runs a bot with these nodes and settings until `want` lines are logged; gives its logs. */
async function runBot(nodes, links, settings, env, want, dir) {
  const bundle = { project: { name: "Server", mode: "test", settings, data: {} }, blocks, libs: [], workspaces: { main: { name: "Main", nodes, links } } };
  const file = path.join(dir, `bundle-${Date.now()}.json`);
  fs.writeFileSync(file, JSON.stringify(bundle));
  const child = spawn(process.execPath, [path.join(__dirname, "..", "circuit-runtime.js"), "--bundle", file, "--deps", deps || dir, "--data", path.join(dir, "data"), "--debug", "--stop-on-input"], { stdio: ["pipe", "pipe", "pipe"], env: { ...process.env, ...env } });
  const seen = [];
  let buf = "";
  let err = "";
  child.stdout.on("data", (d) => {
    buf += d;
    for (const line of buf.split("\n").slice(0, -1)) {
      if (line.startsWith("@@xc ")) {
        const e = JSON.parse(line.slice(5));
        if (e.t === "log") seen.push(`${e.level}: ${e.text}`);
      }
    }
    buf = buf.slice(buf.lastIndexOf("\n") + 1);
  });
  child.stderr.on("data", (d) => (err += d));
  const exited = new Promise((r) => child.on("exit", r));
  const started = Date.now();
  while (seen.filter((s) => !s.includes("Runtime ready")).length < want && Date.now() - started < 20000) await new Promise((r) => setTimeout(r, 50));
  child.stdin.write("stop\n");
  await exited;
  if (err.trim()) seen.push(`stderr: ${err.trim()}`);
  return seen;
}

const n = (block, values = {}) => ({ block: block + "@1", at: [0, 0], values });
const say = (message) => n("core.log", { message, level: "info" });

(async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-server-"));
  const rcon = rconServer();
  const rconPort = await listen(rcon);
  const rcon2 = rconServer("another");
  const rcon2Port = await listen(rcon2);
  const java = javaServer();
  const javaPort = await listen(java);
  const dock = dockerServer();
  const dockPort = await listen(dock);
  const bedrock = dgram.createSocket("udp4");
  await new Promise((r) => bedrock.bind(0, "127.0.0.1", r));
  bedrock.on("message", (msg, from) => {
    const text = Buffer.from("MCPE;§bBedrock world;712;1.21.50;4;30;123;Second line;Survival;1;19132;19133;");
    const out = Buffer.alloc(35 + text.length);
    out[0] = 0x1c;
    msg.copy(out, 1, 1, 9);
    out.writeUInt16BE(text.length, 33);
    text.copy(out, 35);
    bedrock.send(out, from.port, from.address);
  });
  const closed = await listen(net.createServer());
  const free = net.createServer();
  const freePort = await listen(free);
  free.close();

  const chain = [
    ["rc", n("server.rcon_command", { command: "/list" })],
    ["rcLog", say("rcon: ${#rc.answer}")],
    ["mj", n("server.minecraft_status", { edition: "java", address: `127.0.0.1:${javaPort}` })],
    ["mjLog", say("java: ${#mj.online} ${#mj.players}/${#mj.max} ${#mj.names.join('+')} ${#mj.version} ${#mj.motd} ${#mj.icon.length > 0}")],
    ["mb", n("server.minecraft_status", { edition: "bedrock", address: `127.0.0.1:${bedrock.address().port}` })],
    ["mbLog", say("bedrock: ${#mb.online} ${#mb.players}/${#mb.max} ${#mb.version} ${JSON.stringify(#mb.motd)}")],
    ["off", n("server.minecraft_status", { edition: "java", address: `127.0.0.1:${freePort}`, seconds: 2 })],
    ["offLog", say("offline: ${#off.online} ${#off.error}")],
    ["dl", n("server.docker", { action: "list" })],
    ["dlLog", say("docker: ${#dl.containers.map(c => c.name + '=' + c.state).join(',')} ${#dl.containers[0].id}")],
    ["dr", n("server.docker", { action: "restart", container: "web" })],
    ["dg", n("server.docker", { action: "logs", container: "web", lines: 5 })],
    ["dgLog", say("logs: ${JSON.stringify(#dg.logs)}")],
    ["pc", n("server.port_check", { host: "127.0.0.1", port: rconPort })],
    ["pc2", n("server.port_check", { host: "127.0.0.1", port: freePort })],
    ["pcLog", say("ports: ${#pc.open} ${#pc2.open} ${#pc2.error}")],
    // Given on the block instead of Bot › Settings.
    ["rg", n("server.rcon_command", { command: "list", server: "given", host: "127.0.0.1", port: rcon2Port, password: "another" })],
    ["rgLog", say("rcon given: ${#rg.answer}")],
    ["dv", n("server.docker", { action: "list", server: "given", host: `tcp://127.0.0.1:${dockPort}` })],
    ["dvLog", say("docker given: ${#dv.containers.length}")],
    ["us", n("server.server_usage", { where: "here" })],
    ["usLog", say("usage: ${#us.cpu >= 0 && #us.cpu <= 100} ${#us.memory > 0 && #us.memory <= 100} ${#us.disk > 0} ${#us.uptime > 0} ${#us.text.startsWith('CPU ')}")],
  ];
  if (ssh2) {
    chain.push(
      ["sh", n("server.ssh_command", { command: "echo ${who} && whoami" })],
      ["shLog", say("ssh: ${#sh.code} ${JSON.stringify(#sh.output)}")],
      ["su", n("server.server_usage", { where: "ssh" })],
      ["suLog", say("ssh usage: ${#su.cpu} ${#su.memory} ${#su.disk} ${#su.uptime}")],
      ["sg", n("server.ssh_command", { command: "hostname", server: "given", host: "127.0.0.1", user: "bot", password: "secret" })],
      ["sgLog", say("ssh given: ${JSON.stringify(#sg.output)}")],
      ["ug", n("server.server_usage", { where: "given", host: "127.0.0.1", user: "bot", password: "secret" })],
      ["ugLog", say("usage given: ${#ug.disk}")],
    );
  }
  const nodes = { start: n("core.on_start") };
  const links = [];
  let prev = "start";
  for (const [id, node] of chain) {
    nodes[id] = node;
    links.push(`${prev}.next > ${id}.in`);
    prev = id;
  }
  nodes.sh && (nodes.sh.values.who = "x'; rm -rf / #");
  // A wrong RCON password, and an unknown container, in flows of their own.
  nodes.s2 = n("core.on_start");
  nodes.badRcon = n("server.rcon_command", { command: "list", server: "given", host: "127.0.0.1", port: rcon2Port, password: "pw" });
  nodes.s3 = n("core.on_start");
  nodes.badDock = n("server.docker", { action: "stop", container: "nope" });
  links.push("s2.next > badRcon.in", "s3.next > badDock.in");

  // SSH: a stand-in server that answers what it's asked to run.
  let sshServer = null;
  let ran = [];
  const sshSettings = {};
  if (ssh2) {
    const key = ssh2.utils.generateKeyPairSync("ed25519").private;
    sshServer = new ssh2.Server({ hostKeys: [key] }, (client) => {
      client.on("error", () => {});
      client.on("authentication", (ctx) => (ctx.method === "password" && ctx.username === "bot" && ctx.password === "secret" ? ctx.accept() : ctx.reject()));
      client.on("session", (accept) =>
        accept().on("exec", (acc, rej, info) => {
          ran.push(info.command);
          const stream = acc();
          if (info.command.startsWith("head -1 /proc/stat")) stream.write("cpu 100 0 100 800 0 0 0\ncpu 150 0 150 900 0 0 0\nMem: 8000000000 3000000000 1000000000 0 0 6000000000\n/dev/sda1 100000000000 25000000000 75000000000 25% /\n3600.5 7000.1\n");
          else stream.write(`ran ${info.command}\n`);
          stream.exit(0);
          stream.end();
        }),
      );
    });
    Object.assign(sshSettings, { sshHost: "127.0.0.1", sshPort: await listen(sshServer), sshUser: "bot" });
    nodes.sg.values.port = nodes.ug.values.port = sshSettings.sshPort;
  }

  const settings = { serverToolsAllowed: true, serverGivenAllowed: true, rconHost: "127.0.0.1", rconPort, dockerHost: `tcp://127.0.0.1:${dockPort}`, ...sshSettings };
  const env = { XC_SECRET_RCONPASSWORD: "pw", XC_SECRET_SSHPASSWORD: "secret" };
  const seen = await runBot(nodes, links, settings, env, chain.filter(([id]) => id.endsWith("Log")).length + 2, dir);
  const all = seen.join("\n");
  const has = (w) => assert.ok(seen.some((s) => s.endsWith(w)), `logged "${w}":\n${all}`);
  has("rcon: There are 2 of 10 players online: Alex, Bea");
  has("java: true 3/20 Alex+Bea 1.21.4 Welcome home true");
  has('bedrock: true 4/30 1.21.50 "Bedrock world\\nSecond line"');
  assert.ok(seen.some((s) => s.includes("offline: false") && s.includes(`nothing answers at 127.0.0.1:${freePort}`)), all);
  has("docker: web=running,db=exited abcdef123456");
  has('logs: "started\\nwarning: slow"');
  has("ports: true false refused");
  has("usage: true true true true true");
  has("rcon given: There are 2 of 10 players online: Alex, Bea");
  has("docker given: 2");
  assert.ok(seen.some((s) => s.startsWith("error:") && s.includes("RCON password is wrong")), all);
  assert.ok(seen.some((s) => s.startsWith("error:") && s.includes("No such container: nope")), all);
  if (ssh2) {
    // The ${…} value went to the shell as one quoted word.
    assert.deepStrictEqual(ran[0], `echo 'x'\\''; rm -rf / #' && whoami`);
    has(`ssh: 0 "ran echo 'x'\\\\''; rm -rf / #' && whoami"`);
    has("ssh usage: 50 25 25 3601");
    has('ssh given: "ran hostname"');
    has("usage given: 25");
    // The same address with another key: refused.
    sshServer.close();
    const other = new ssh2.Server({ hostKeys: [ssh2.utils.generateKeyPairSync("ed25519").private] }, (c) => c.on("error", () => {}).on("authentication", (x) => x.accept()));
    await new Promise((r) => other.listen(sshSettings.sshPort, "127.0.0.1", r));
    const again = await runBot({ start: n("core.on_start"), sh: n("server.ssh_command", { command: "whoami" }) }, ["start.next > sh.in"], settings, env, 1, dir);
    assert.ok(again.some((s) => s.includes("server's key isn't the one it had before")), again.join("\n"));
    other.close();
  } else {
    console.log("  (SSH skipped: ssh2 isn't in XC_TEST_DEPS)");
  }

  // Not allowed in Bot › Settings: the powerful blocks refuse.
  const ctx = { runtime: { setting: () => false, secret: () => "" } };
  for (const id of ["server.ssh_command", "server.rcon_command", "server.docker"]) {
    const block = require(blocks[id].file);
    await assert.rejects(block.run({ fields: { command: "x", action: "list" }, ctx }), /Server tools are off/, id);
  }
  await assert.rejects(require(blocks["server.server_usage"].file).run({ fields: { where: "ssh" }, ctx }), /Server tools are off/);
  // Allowed, but not servers given on blocks: those refuse, before connecting anywhere.
  const toolsOnly = { runtime: { setting: (k) => k === "serverToolsAllowed", secret: () => "" }, require: (m) => require(m), store: { ns: () => ({ get: async () => null }) } };
  for (const [id, fields] of [
    ["server.ssh_command", { command: "x", server: "given", host: "127.0.0.1" }],
    ["server.rcon_command", { command: "x", server: "given", host: "127.0.0.1" }],
    ["server.docker", { action: "list", server: "given", host: "tcp://127.0.0.1:1" }],
    ["server.server_usage", { where: "given", host: "127.0.0.1" }],
  ]) {
    await assert.rejects(require(blocks[id].file).run({ fields, ctx: toolsOnly }), /Servers given on a block are off/, id);
  }

  for (const s of [rcon, rcon2, java, dock]) s.close();
  bedrock.close();
  fs.rmSync(dir, { recursive: true, force: true });
  console.log("server tools test passed");
  process.exit(0);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
