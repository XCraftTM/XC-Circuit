// A check of a running host, from outside: node host/test/running.js <address> <pairing code>
//
// Pairs a device, keeps a project there with a file of several megabytes
// (more than Git sends in one piece), gets it back, and removes the device
// again. For the container in CI, and for a host behind its proxy.
"use strict";

const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execFile } = require("node:child_process");

const [address, code] = process.argv.slice(2);
if (!address || !code) {
  console.error("node host/test/running.js <address> <pairing code>");
  process.exit(2);
}

function connect() {
  const ws = new WebSocket(address.replace(/^http/, "ws") + "/ws");
  const got = [];
  const waiting = [];
  ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    const at = waiting.findIndex((w) => w.t === m.t || m.t === "error");
    if (at >= 0) waiting.splice(at, 1)[0].resolve(m);
    else got.push(m);
  };
  return {
    ready: new Promise((resolve, reject) => {
      ws.onopen = resolve;
      ws.onerror = () => reject(new Error(`couldn't connect to ${address}`));
    }),
    ask(message, want) {
      ws.send(JSON.stringify(message));
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error(`no ${want} within 15 s`)), 15000);
        const take = (m) => {
          clearTimeout(timer);
          m.t === want ? resolve(m) : reject(new Error(`${message.t}: ${m.why || m.t}`));
        };
        const at = got.findIndex((m) => m.t === want || m.t === "error");
        if (at >= 0) take(got.splice(at, 1)[0]);
        else waiting.push({ t: want, resolve: take });
      });
    },
    close: () => ws.close(),
  };
}

(async () => {
  const status = await (await fetch(`${address}/status`)).json();
  assert.equal(status.name, "xc-circuit-host");
  assert.ok(status.can.includes("projects"), "this check is for a host in full mode");

  const device = connect();
  await device.ready;
  const paired = await device.ask({ t: "pair", code, name: "Check", key: "" }, "paired");
  const auth = `Authorization: Basic ${Buffer.from(`check:${paired.token}`).toString("base64")}`;
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-host-running-"));
  const name = `check-${crypto.randomBytes(4).toString("hex")}`;
  const url = `${address}/git/${name}.git`;
  const git = (cwd, ...args) =>
    new Promise((resolve, reject) => {
      const all = ["-c", `http.extraHeader=${auth}`, "-c", "user.name=Check", "-c", "user.email=check@example.com", "-c", "core.autocrlf=false", "-c", "credential.helper=", ...args];
      execFile("git", all, { cwd, encoding: "utf8", env: { ...process.env, GIT_TERMINAL_PROMPT: "0" }, timeout: 120000 }, (error, stdout, stderr) => (error ? reject(new Error(`git ${args.join(" ")}: ${stderr || error.message}`)) : resolve(stdout)));
    });
  try {
    const work = path.join(dir, "work");
    fs.mkdirSync(work);
    const big = crypto.randomBytes(6_000_000);
    fs.writeFileSync(path.join(work, "picture.bin"), big);
    fs.writeFileSync(path.join(work, "circuit.project.json"), "{}\n");
    await git(work, "init", "-q", "-b", "main");
    await git(work, "add", ".");
    await git(work, "commit", "-q", "-m", "A project with a big file");
    await git(work, "push", "-q", url, "main");
    const copy = path.join(dir, "copy");
    await git(dir, "clone", "-q", url, copy);
    assert.ok(fs.readFileSync(path.join(copy, "picture.bin")).equals(big), "the big file came back as it went");
    const listed = await device.ask({ t: "projects" }, "projects");
    assert.ok(listed.projects.includes(name));
    console.log(`ok: paired, stored 6 MB as ${name} and got it back`);
  } finally {
    await device.ask({ t: "device.remove", id: paired.device }, "device.removed").catch(() => {});
    device.close();
    fs.rmSync(dir, { recursive: true, force: true });
  }
  process.exit(0);
})().catch((e) => {
  console.error(`FAILED: ${e.message}`);
  process.exit(1);
});
