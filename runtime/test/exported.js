// An exported bot (index.js: the bundle, then the runtime) watches itself:
// started again after a crash and when it asks to be, not when it can't start
// or was stopped. And one block whose setup fails doesn't keep it from starting.
const fs = require("fs");
const os = require("os");
const path = require("path");
const assert = require("assert");
const { spawn } = require("child_process");
const { restartWait, dotEnvValue } = require("../circuit-runtime.js");

// The wait grows while the bot keeps crashing right after it started.
assert.deepStrictEqual([1, 2, 3, 4, 5, 9].map(restartWait), [5, 10, 20, 40, 60, 60]);

// .env values: plain ones as they are, quoted ones without their quotes.
assert.strictEqual(dotEnvValue("abc # mine"), "abc");
assert.strictEqual(dotEnvValue('"#secret \\"1\\" \\\\ x" # note'), '#secret "1" \\ x');
assert.strictEqual(dotEnvValue("'single # kept'"), "single # kept");
assert.strictEqual(dotEnvValue('"C:\\path"'), "C:\\path");

const runtime = fs.readFileSync(path.join(__dirname, "..", "circuit-runtime.js"), "utf8");

/** An exported bot whose one event block does `listen` (JavaScript text) when the bot starts. */
function exported(listen, extra = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-exported-"));
  const event = `module.exports = { id: "t.go", kind: "event", name: "Go", outputs: {}, listen(ctx, emit) { const fs = require("fs"); const path = require("path"); const file = path.join(ctx.runtime.dataDir, "..", "starts.txt"); const starts = (fs.existsSync(file) ? Number(fs.readFileSync(file, "utf8")) : 0) + 1; fs.writeFileSync(file, String(starts)); ${listen} } };`;
  const broken = `module.exports = { id: "t.broken", kind: "action", name: "Broken", setup() { throw new Error("no good"); }, run() {} };`;
  const bundle = {
    project: { name: "T", mode: "none", settings: {} },
    blocks: { "t.go": { source: event, file: "t/go.js" }, "t.broken": { source: broken, file: "t/broken.js" } },
    libs: [],
    workspaces: { main: { name: "Main", nodes: { a: { block: "t.go@1", at: [0, 0] }, b: { block: "t.broken@1", at: [0, 0] } }, links: [] } },
    ...extra,
  };
  fs.writeFileSync(path.join(dir, "index.js"), `const __XC_BUNDLE__ = ${JSON.stringify(bundle)};\n\n${runtime}`);
  return dir;
}

function run(dir, args = [], env = {}) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, ["index.js", ...args], { cwd: dir, env: { ...process.env, ...env }, stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    child.stdout.on("data", (d) => (out += d));
    child.stderr.on("data", (d) => (out += d));
    const timer = setTimeout(() => child.kill(), 60_000);
    child.on("exit", (code) => {
      clearTimeout(timer);
      const file = path.join(dir, "starts.txt");
      resolve({ code, out, starts: fs.existsSync(file) ? Number(fs.readFileSync(file, "utf8")) : 0 });
    });
  });
}

(async () => {
  // Asks to be started again once, then stops: two starts, and the failed setup is only reported.
  let dir = exported(`if (starts === 1) ctx.bot.restart(); else ctx.bot.stop();`);
  let r = await run(dir);
  assert.strictEqual(r.starts, 2, r.out);
  assert.strictEqual(r.code, 0, r.out);
  assert.ok(r.out.includes("Broken") && r.out.includes("its setup failed: no good"), r.out);
  fs.rmSync(dir, { recursive: true, force: true });

  // Crashes the first time: started again after the wait, then stops.
  dir = exported(`if (starts === 1) setTimeout(() => { throw new Error("boom"); }, 10); else ctx.bot.stop();`);
  const t0 = Date.now();
  r = await run(dir);
  assert.strictEqual(r.starts, 2, r.out);
  assert.ok(r.out.includes("Bot crashed") && r.out.includes("Starting it again in 5 s"), r.out);
  assert.ok(Date.now() - t0 >= 4500, "it waited before starting again");
  fs.rmSync(dir, { recursive: true, force: true });

  // --no-restart (and XC_NO_RESTART=1): one process, as before; a crash ends it.
  for (const [args, env] of [[["--no-restart"], {}], [[], { XC_NO_RESTART: "1" }]]) {
    dir = exported(`setTimeout(() => { throw new Error("boom"); }, 10);`);
    r = await run(dir, args, env);
    assert.strictEqual(r.starts, 1, r.out);
    assert.strictEqual(r.code, 1, r.out);
    fs.rmSync(dir, { recursive: true, force: true });
  }

  // A bot that can't start (its library fails to log in) isn't started again and again.
  const lib = `module.exports = { id: "lib", setup() { return {}; }, async start() { throw new Error("Login failed: no token"); } };`;
  dir = exported(`ctx.log.info("listening");`, { libs: [{ source: lib, file: "t/lib.js" }] });
  r = await run(dir);
  assert.strictEqual(r.starts, 1, r.out);
  fs.rmSync(dir, { recursive: true, force: true });

  console.log("exported test passed");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
