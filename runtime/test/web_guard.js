// Fetching what people point the bot at: which addresses count as private,
// the size limit (also without a Content-Length), redirects checked, and
// HTTP Request in a running bot: an address the author typed goes anywhere,
// one filled in from ${…} only to the public web unless the block allows it.
// Usage: node runtime/test/web_guard.js
"use strict";

const fs = require("fs");
const os = require("os");
const http = require("http");
const path = require("path");
const assert = require("assert");
const { spawn } = require("child_process");
const { isPrivateAddress } = require("../circuit-runtime.js");

const root = path.join(__dirname, "..", "..");
const blocks = {};
for (const folder of ["web", "events", "debug"]) {
  const dir = path.join(root, "plugins", "xc.core", "blocks", folder);
  for (const f of fs.readdirSync(dir)) blocks[require(path.join(dir, f)).id] = { source: fs.readFileSync(path.join(dir, f), "utf8"), file: path.join(dir, f) };
}

/** Runs a bot until `want` lines are logged; gives its logs. */
async function runBot(nodes, links, want, dir) {
  const bundle = { project: { name: "Guard", mode: "test", settings: {}, data: {} }, blocks, libs: [], workspaces: { main: { name: "Main", nodes, links } } };
  const file = path.join(dir, "bundle.json");
  fs.writeFileSync(file, JSON.stringify(bundle));
  const child = spawn(process.execPath, [path.join(__dirname, "..", "circuit-runtime.js"), "--bundle", file, "--deps", dir, "--data", path.join(dir, "data"), "--debug", "--stop-on-input"], { stdio: ["pipe", "pipe", "pipe"] });
  const seen = [];
  let buf = "";
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
  const exited = new Promise((r) => child.on("exit", r));
  const started = Date.now();
  while (seen.filter((s) => !s.includes("Runtime ready")).length < want && Date.now() - started < 20000) await new Promise((r) => setTimeout(r, 50));
  child.stdin.write("stop\n");
  await exited;
  return seen;
}

const n = (block, values = {}) => ({ block: block + "@1", at: [0, 0], values });

(async () => {
  // Private: this computer, private networks, link-local (cloud metadata), CGNAT, multicast.
  for (const ip of ["127.0.0.1", "10.1.2.3", "172.16.0.1", "172.31.255.255", "192.168.1.1", "169.254.169.254", "100.64.0.1", "0.0.0.0", "224.0.0.1", "::1", "::", "fc00::1", "fd12::1", "fe80::1", "::ffff:127.0.0.1", "[::1]"]) {
    assert.ok(isPrivateAddress(ip), `${ip} is private`);
  }
  for (const ip of ["1.1.1.1", "8.8.8.8", "172.32.0.1", "100.128.0.1", "2606:4700:4700::1111"]) {
    assert.ok(!isPrivateAddress(ip), `${ip} is public`);
  }

  // A server on this computer: small and big answers, one without a length, and a redirect.
  const server = http.createServer((req, res) => {
    if (req.url === "/ok") return res.end(JSON.stringify({ ok: "yes" }));
    if (req.url === "/big") return res.end("x".repeat(3 * 1024 * 1024));
    if (req.url === "/stream") {
      res.writeHead(200, { "Content-Type": "text/plain" });
      let sent = 0;
      const more = () => {
        while (sent < 3 * 1024 * 1024) {
          sent += 64 * 1024;
          if (!res.write("y".repeat(64 * 1024))) return res.once("drain", more);
        }
        res.end();
      };
      return more();
    }
    if (req.url === "/away") return res.writeHead(302, { Location: "http://127.0.0.1:1/secret" }).end();
    res.writeHead(404).end();
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const base = `http://127.0.0.1:${server.address().port}`;

  // The guarded fetch the blocks use (ctx.util.fetch).
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-guard-"));
  const get = require("../circuit-runtime.js").guardedFetch;
  const mb = 1024 * 1024;
  let r = await get(`${base}/ok`, {}, { publicOnly: false, maxBytes: mb });
  assert.deepStrictEqual(JSON.parse(r.body.toString()), { ok: "yes" });
  await assert.rejects(get(`${base}/ok`, {}, { publicOnly: true }), /private network/);
  await assert.rejects(get("http://localhost:1/", {}, { publicOnly: true }), /private network/, "a name that leads to this computer");
  await assert.rejects(get(`${base}/big`, {}, { maxBytes: mb }), /more than 1 MB/, "too big, by its length");
  await assert.rejects(get(`${base}/stream`, {}, { maxBytes: mb }), /more than 1 MB/, "too big, without a length");
  await assert.rejects(get("file:///etc/passwd", {}, { publicOnly: true }), /Only web addresses/);
  r = await get(`${base}/stream`, {}, { maxBytes: 4 * mb });
  assert.strictEqual(r.body.length, 3 * mb);

  // HTTP Request in a bot: typed, filled in from ${…}, and filled in but allowed.
  const nodes = {
    s1: n("core.on_start"),
    typed: n("core.http_request", { url: `${base}/ok` }),
    log1: n("core.log", { message: "typed: ${#typed.data.ok}" }),
    s2: n("core.on_start"),
    filled: n("core.http_request", { url: `http://\${'127.0.0.1'}:${server.address().port}/ok` }),
    s3: n("core.on_start"),
    allowed: n("core.http_request", { url: `http://\${'127.0.0.1'}:${server.address().port}/ok`, privateOk: true }),
    log3: n("core.log", { message: "allowed: ${#allowed.data.ok}" }),
    s4: n("core.on_start"),
    big: n("core.http_request", { url: `${base}/big`, maxMb: 1 }),
  };
  const links = ["s1.next > typed.in", "typed.next > log1.in", "s2.next > filled.in", "s3.next > allowed.in", "allowed.next > log3.in", "s4.next > big.in"];
  const seen = await runBot(nodes, links, 4, dir);
  const all = seen.join("\n");
  assert.ok(seen.some((s) => s.endsWith("typed: yes")), all);
  assert.ok(seen.some((s) => s.endsWith("allowed: yes")), all);
  assert.ok(seen.some((s) => s.startsWith("error:") && s.includes("private network")), all);
  assert.ok(seen.some((s) => s.startsWith("error:") && s.includes("more than 1 MB")), all);

  server.close();
  fs.rmSync(dir, { recursive: true, force: true });
  console.log("web guard test passed");
  process.exit(0);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
