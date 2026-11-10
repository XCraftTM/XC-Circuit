// Tests for the Host: node host/test/host.js
"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync, execFile } = require("node:child_process");
const { test } = require("node:test");
const { start, settings, LIMITS, PROTOCOL } = require("../server.js");

const quiet = () => {};

/// An app's end of a connection: `next("type")` waits for that message.
function connect(port) {
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  const got = [];
  const waiting = [];
  let closed = false;
  ws.onmessage = (e) => {
    const message = JSON.parse(e.data);
    const at = waiting.findIndex((w) => w.type === message.t);
    if (at >= 0) waiting.splice(at, 1)[0].resolve(message);
    else got.push(message);
  };
  ws.onclose = () => {
    closed = true;
    for (const w of waiting.splice(0)) w.reject(new Error(`closed while waiting for ${w.type}`));
  };
  return {
    ready: new Promise((resolve, reject) => {
      ws.onopen = resolve;
      ws.onerror = () => reject(new Error("couldn't connect"));
    }),
    send: (message) => ws.send(JSON.stringify(message)),
    raw: (text) => ws.send(text),
    next(type) {
      const at = got.findIndex((m) => m.t === type);
      if (at >= 0) return Promise.resolve(got.splice(at, 1)[0]);
      if (closed) return Promise.reject(new Error(`closed, no ${type}`));
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error(`no ${type} within 5 s`)), 5000);
        waiting.push({ type, resolve: (m) => (clearTimeout(timer), resolve(m)), reject: (e) => (clearTimeout(timer), reject(e)) });
      });
    },
    closed: () => closed,
    close: () => ws.close(),
  };
}

async function app(port) {
  const c = connect(port);
  await c.ready;
  await c.next("status");
  return c;
}

function dataDir(name) {
  const dir = path.join(os.tmpdir(), `xc-host-${name}-${process.pid}`);
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

test("settings are read and checked", () => {
  assert.deepEqual(settings({}), { mode: "full", port: 7421, data: "/data", trusted: [] });
  assert.deepEqual(settings({ XC_HOST_MODE: "Relay", XC_HOST_PORT: "9000", XC_HOST_TRUSTED_PROXY: "10.0.0.2, private" }), { mode: "relay", port: 9000, data: "/data", trusted: ["10.0.0.2", "private"] });
  assert.throws(() => settings({ XC_HOST_MODE: "both" }), /full or relay/);
  assert.throws(() => settings({ XC_HOST_PORT: "http" }), /port number/);
});

test("a relay says what it is and keeps nothing", async () => {
  const host = await start({ mode: "relay", port: 0, address: "127.0.0.1", trusted: [], log: quiet });
  try {
    const status = await (await fetch(`http://127.0.0.1:${host.port}/status`)).json();
    assert.deepEqual(status, { name: "xc-circuit-host", version: status.version, protocol: PROTOCOL, mode: "relay", can: ["relay"] });
    assert.equal((await fetch(`http://127.0.0.1:${host.port}/git/x.git/info/refs`)).status, 404, "no projects on a relay");
    const a = await app(host.port);
    a.send({ t: "invite" });
    assert.match((await a.next("error")).why, /only passes live sessions on/);
    a.send({ t: "vault.get", project: "p" });
    assert.match((await a.next("error")).why, /only passes live sessions on/);
    a.close();
  } finally {
    await host.close();
  }
});

test("a session's messages go from the host to the guests and back", async () => {
  const host = await start({ mode: "relay", port: 0, address: "127.0.0.1", trusted: [], log: quiet });
  try {
    const [owner, one, two, stranger] = await Promise.all([app(host.port), app(host.port), app(host.port), app(host.port)]);
    owner.send({ t: "open" });
    const { room } = await owner.next("opened");
    assert.match(room, /^[0-9a-f]{24}$/);
    stranger.send({ t: "join", room: "nope" });
    assert.match((await stranger.next("error")).why, /isn't open/);
    one.send({ t: "join", room });
    const first = (await one.next("joined")).you;
    assert.deepEqual(await owner.next("peer"), { t: "peer", id: first, on: true });
    two.send({ t: "join", room });
    const second = (await two.next("joined")).you;
    await owner.next("peer");

    // What goes through is the apps' business: the host only passes it on.
    one.send({ t: "to", data: { sealed: "from one" } });
    assert.deepEqual(await owner.next("from"), { t: "from", id: first, data: { sealed: "from one" } });
    owner.send({ t: "to", id: second, data: "only two" });
    assert.deepEqual(await two.next("from"), { t: "from", id: "host", data: "only two" });
    owner.send({ t: "to", data: "everyone" });
    assert.equal((await one.next("from")).data, "everyone");
    assert.equal((await two.next("from")).data, "everyone");

    // A guest can't be let in twice, and can't send others away.
    one.send({ t: "kick", id: second });
    assert.match((await one.next("error")).why, /Only the session's host/);
    owner.send({ t: "kick", id: second });
    await two.next("closed");
    assert.deepEqual(await owner.next("peer"), { t: "peer", id: second, on: false });

    one.send({ t: "ping" });
    await one.next("pong");
    one.raw("not json");
    assert.match((await one.next("error")).why, /JSON/);

    // Without its host a session is over.
    owner.close();
    await one.next("closed");
    one.send({ t: "to", data: 1 });
    assert.match((await one.next("error")).why, /no session/);
    for (const c of [one, two, stranger]) c.close();
  } finally {
    await host.close();
  }
});

test("too much is refused", async () => {
  const host = await start({ mode: "relay", port: 0, address: "127.0.0.1", trusted: [], log: quiet });
  try {
    const opened = [];
    for (let i = 0; i < LIMITS.roomsPerAddress; i++) {
      const c = await app(host.port);
      c.send({ t: "open" });
      await c.next("opened");
      opened.push(c);
    }
    const more = await app(host.port);
    more.send({ t: "open" });
    assert.match((await more.next("error")).why, /Too many sessions/);
    more.raw(JSON.stringify({ t: "to", data: "x".repeat(LIMITS.message + 1) }));
    await new Promise((r) => setTimeout(r, 300));
    assert.ok(more.closed(), "a message over the limit ends the connection");
    for (const c of opened) c.close();
  } finally {
    await host.close();
  }
});

test("full mode needs its data folder", async () => {
  await assert.rejects(start({ mode: "full", port: 0, data: path.join(os.tmpdir(), "xc-host-not-there"), trusted: [], log: quiet }), /data folder .* isn't there/);
});

test("devices are paired, and keep secrets the host can't read", async () => {
  const dir = dataDir("full");
  const lines = [];
  let host = await start({ mode: "full", port: 0, address: "127.0.0.1", data: dir, trusted: [], log: (l) => lines.push(l) });
  try {
    const status = await (await fetch(`http://127.0.0.1:${host.port}/status`)).json();
    assert.deepEqual(status.can, ["relay", "projects", "vault"]);
    const code = host.setupCode();
    assert.match(code, /^[A-Z2-9]{4}-[A-Z2-9]{4}$/);
    assert.ok(lines.some((l) => l.includes(code)), "the first code is in the log");

    const laptop = await app(host.port);
    laptop.send({ t: "vault.get", project: "p" });
    assert.match((await laptop.next("error")).why, /Pair this device first/);
    laptop.send({ t: "pair", code: "AAAA-AAAA", name: "Laptop" });
    assert.match((await laptop.next("error")).why, /isn't right/);
    laptop.send({ t: "pair", code: code.toLowerCase(), name: "Laptop", key: "laptop-public-key" });
    const paired = await laptop.next("paired");
    assert.match(paired.token, /^[0-9a-f]{64}$/);
    assert.equal(host.setupCode(), null, "used once");
    const again = await app(host.port);
    again.send({ t: "pair", code, name: "Thief" });
    assert.match((await again.next("error")).why, /isn't right/);
    again.close();
    assert.ok(!fs.readFileSync(path.join(dir, "host.sqlite")).includes(paired.token), "the token is not kept as it is");

    // Secrets: a blob only the devices can read, with a count that keeps
    // two devices from overwriting each other.
    laptop.send({ t: "vault.put", project: "0123abcd", blob: "sealed-1", rev: 0 });
    assert.deepEqual(await laptop.next("vault.kept"), { t: "vault.kept", project: "0123abcd", rev: 1 });
    laptop.send({ t: "vault.put", project: "0123abcd", blob: "sealed-old", rev: 0 });
    assert.match((await laptop.next("error")).why, /newer secrets/);
    laptop.send({ t: "vault.put", project: "../x", blob: "b", rev: 0 });
    assert.match((await laptop.next("error")).why, /project's id/);

    // A second device comes in with a code from the first, and gets the
    // key the first wrapped for it.
    laptop.send({ t: "invite" });
    const invited = await laptop.next("invited");
    const tablet = await app(host.port);
    tablet.send({ t: "pair", code: invited.code, name: "Tablet", key: "tablet-public-key" });
    const second = await tablet.next("paired");
    laptop.send({ t: "devices" });
    const listed = await laptop.next("devices");
    assert.deepEqual(listed.devices.map((d) => [d.name, d.key]), [["Laptop", "laptop-public-key"], ["Tablet", "tablet-public-key"]]);
    assert.equal(listed.you, paired.device);
    laptop.send({ t: "key.put", for: second.device, blob: "vault-key-wrapped-for-tablet" });
    assert.deepEqual(await tablet.next("keys"), { t: "keys", keys: [{ from: paired.device, blob: "vault-key-wrapped-for-tablet" }] });
    tablet.send({ t: "vault.get", project: "0123abcd" });
    assert.deepEqual(await tablet.next("vault"), { t: "vault", project: "0123abcd", blob: "sealed-1", rev: 1 });
    tablet.close();

    // It all outlives a restart; a device says who it is with its token.
    laptop.close();
    await host.close();
    host = await start({ mode: "full", port: 0, address: "127.0.0.1", data: dir, trusted: [], log: quiet });
    assert.equal(host.setupCode(), null, "no setup code once a device is paired");
    const back = await app(host.port);
    back.send({ t: "hello", token: "0".repeat(64) });
    assert.match((await back.next("error")).why, /isn't paired/);
    back.send({ t: "hello", token: paired.token });
    assert.deepEqual(await back.next("welcome"), { t: "welcome", device: paired.device, name: "Laptop" });
    back.send({ t: "vault.get", project: "0123abcd" });
    assert.equal((await back.next("vault")).blob, "sealed-1");

    // A removed device is out at once.
    const gone = await app(host.port);
    gone.send({ t: "hello", token: second.token });
    await gone.next("welcome");
    back.send({ t: "device.remove", id: second.device });
    assert.equal((await back.next("device.removed")).ok, true);
    await new Promise((r) => setTimeout(r, 300));
    assert.ok(gone.closed());
    back.close();
  } finally {
    await host.close();
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("projects are Git repositories, for paired devices only", { skip: spawnSync("git", ["--version"]).status !== 0 && "needs Git" }, async () => {
  const dir = dataDir("git");
  const host = await start({ mode: "full", port: 0, address: "127.0.0.1", data: dir, trusted: [], log: quiet });
  try {
    const device = await app(host.port);
    device.send({ t: "pair", code: host.setupCode(), name: "Laptop" });
    const { token } = await device.next("paired");
    const url = `http://127.0.0.1:${host.port}/git/my-bot.git`;
    assert.equal((await fetch(`${url}/info/refs?service=git-upload-pack`)).status, 401);
    assert.equal((await fetch(`${url}/info/refs?service=git-upload-pack`, { headers: { Authorization: `Basic ${Buffer.from("x:wrong").toString("base64")}` } })).status, 401);
    const auth = `Authorization: Basic ${Buffer.from(`laptop:${token}`).toString("base64")}`;
    const bad = await fetch(`http://127.0.0.1:${host.port}/git/..%2Fx.git/info/refs`, { headers: { Authorization: auth.split(": ")[1] } });
    assert.equal(bad.status, 404);

    const work = path.join(dir, "work");
    // Not spawnSync: the host answers from this same process.
    const git = (cwd, ...args) =>
      new Promise((resolve, reject) => {
        const all = ["-c", `http.extraHeader=${auth}`, "-c", "user.name=T", "-c", "user.email=t@example.com", "-c", "core.autocrlf=false", "-c", "credential.helper=", ...args];
        execFile("git", all, { cwd, encoding: "utf8", env: { ...process.env, GIT_TERMINAL_PROMPT: "0" }, timeout: 20000 }, (error, stdout, stderr) => (error ? reject(new Error(`git ${args.join(" ")}: ${stderr || error.message}`)) : resolve(stdout)));
      });
    fs.mkdirSync(work);
    // Another paired device is connected: it hears when something is stored.
    device.send({ t: "invite" });
    const other = await app(host.port);
    other.send({ t: "pair", code: (await device.next("invited")).code, name: "Tablet" });
    await other.next("paired");
    await git(work, "init", "-q", "-b", "main");
    fs.writeFileSync(path.join(work, "circuit.project.json"), '{ "name": "Bot" }\n');
    await git(work, "add", ".");
    await git(work, "commit", "-q", "-m", "First");
    await git(work, "push", "-q", url, "main");
    const copy = path.join(dir, "copy");
    await git(dir, "clone", "-q", url, copy);
    assert.equal(fs.readFileSync(path.join(copy, "circuit.project.json"), "utf8"), '{ "name": "Bot" }\n');
    const changed = await other.next("project.changed");
    assert.equal(changed.project, "my-bot");
    assert.ok(changed.by && changed.by !== undefined);
    other.close();
    device.send({ t: "projects" });
    assert.deepEqual((await device.next("projects")).projects, ["my-bot"]);

    // As a reverse proxy sends an upload: nothing may come back before all
    // of it is there, or the proxy stops sending and cuts the connection.
    const net = require("node:net");
    const body = Buffer.alloc(3_000_000, 1);
    const early = await new Promise((resolve, reject) => {
      const socket = net.connect(host.port, "127.0.0.1");
      let got = 0;
      let before = -1;
      socket.on("data", (chunk) => (got += chunk.length));
      socket.on("error", reject);
      socket.on("connect", () => {
        socket.write(`POST /git/my-bot.git/git-receive-pack HTTP/1.1\r\nHost: x\r\n${auth}\r\nContent-Type: application/x-git-receive-pack-request\r\nContent-Length: ${body.length}\r\nConnection: close\r\n\r\n`);
        socket.write(body.subarray(0, 1000));
        setTimeout(() => {
          before = got;
          socket.write(body.subarray(1000));
        }, 700);
      });
      socket.on("close", () => resolve({ before, got }));
    });
    assert.equal(early.before, 0, "no answer while the upload is still coming");
    assert.ok(early.got > 0, "and one once it is there");
    device.close();
  } finally {
    await host.close();
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
