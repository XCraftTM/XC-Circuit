// Build Canvas in a running bot: its fonts come along as assets, ${…} in the
// design are filled in, and the picture is a PNG with the design's size and
// colours. Needs @napi-rs/canvas in XC_TEST_DEPS (a folder whose
// node_modules has it); without it, this says so and passes.
// Usage: XC_TEST_DEPS=<dir> node runtime/test/canvas.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const Module = require("module");
const assert = require("assert");
const { spawn } = require("child_process");

const deps = process.env.XC_TEST_DEPS ? path.resolve(process.env.XC_TEST_DEPS) : null;
let napi;
try {
  napi = Module.createRequire(path.join(deps || __dirname, "package.json"))("@napi-rs/canvas");
} catch {
  console.log("canvas test skipped: @napi-rs/canvas isn't in XC_TEST_DEPS");
  process.exit(0);
}

const root = path.join(__dirname, "..", "..");
const blocks = {};
for (const sub of ["design", "events", "code", "debug"]) {
  const folder = path.join(root, "plugins", "xc.core", "blocks", sub);
  for (const f of fs.readdirSync(folder)) {
    const file = path.join(folder, f);
    const src = fs.readFileSync(file, "utf8");
    const id = /id:\s*"([^"]+)"/.exec(src)[1];
    const entry = { source: src, file };
    // As the editor bundles them: the files a block lists in requires.assets.
    const block = require(file);
    const assets = (block.requires && block.requires.assets) || [];
    if (assets.length) entry.assets = Object.fromEntries(assets.map((a) => [a, fs.readFileSync(path.join(path.dirname(file), a)).toString("base64")]));
    blocks[id] = entry;
  }
}

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-canvas-"));
const out = path.join(dir, "card.png");
const n = (block, values = {}) => ({ block: block + "@1", at: [0, 0], values });
const design = {
  width: 400,
  height: 200,
  background: "#102030",
  layers: [
    { type: "rect", name: "Box", x: 0, y: 0, w: 200, h: 200, color: "#FF0000" },
    { type: "circle", name: "Dot", x: 250, y: 50, w: 100, h: 100, color: "${dot}" },
    { type: "text", name: "Name", x: 210, y: 160, w: 180, text: "${who} 🎉", font: "bold", size: 24, color: "#FFFFFF", shrink: true },
    { type: "bar", name: "Bar", x: 0, y: 180, w: 200, h: 20, value: "${xp}", max: "100", color: "#00FF00", back: "#000000" },
    { type: "image", name: "Missing", x: 0, y: 0, w: 10, h: 10, src: path.join(dir, "nope.png") },
    // Filled in from ${…}: only a file in the bot's files folder, not one anywhere on the disk.
    { type: "image", name: "Outside", x: 0, y: 0, w: 10, h: 10, src: "${outside}" },
    { type: "image", name: "Kept", x: 10, y: 0, w: 10, h: 10, src: "${kept}" },
    { type: "rect", name: "Hidden", x: 0, y: 0, w: 400, h: 200, color: "#FFFFFF", hidden: true },
  ],
};
const nodes = {
  start: n("core.on_start"),
  draw: n("core.build_canvas", { canvas: design, name: "card" }),
  save: n("core.run_code", { inputs: "file, url", outputs: "", code: `require("fs").writeFileSync(${JSON.stringify(out)}, inputs.file.attachment); ctx.log.info("saved " + inputs.file.name + " " + inputs.url);` }),
};
// Values the design's ${…} use, set as run variables before drawing.
nodes.draw.values.dot = "#0000FF";
nodes.draw.values.who = "Alex";
nodes.draw.values.xp = 50;
// A picture outside the files folder, and one in it.
const tiny = napi.createCanvas(2, 2).toBuffer("image/png");
fs.writeFileSync(path.join(dir, "outside.png"), tiny);
fs.mkdirSync(path.join(dir, "data", "files"), { recursive: true });
fs.writeFileSync(path.join(dir, "data", "files", "pic.png"), tiny);
nodes.draw.values.outside = path.join(dir, "outside.png");
nodes.draw.values.kept = "pic.png";
const links = ["start.next > draw.in", "draw.next > save.in", "draw.file > save.file", "draw.url > save.url"];
const bundle = { project: { name: "Canvas", mode: "test", settings: {}, data: {} }, blocks, libs: [], workspaces: { main: { name: "Main", nodes, links } } };
const file = path.join(dir, "bundle.json");
fs.writeFileSync(file, JSON.stringify(bundle));

const child = spawn(process.execPath, [path.join(__dirname, "..", "circuit-runtime.js"), "--bundle", file, "--deps", deps, "--data", path.join(dir, "data"), "--debug", "--stop-on-input"], { stdio: ["pipe", "pipe", "pipe"] });
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
let err = "";
child.stderr.on("data", (d) => (err += d));
const exited = new Promise((r) => child.on("exit", r));

(async () => {
  const started = Date.now();
  while (!seen.some((s) => s.includes("saved")) && Date.now() - started < 20000) await new Promise((r) => setTimeout(r, 100));
  child.stdin.write("stop\n");
  await exited;
  assert.ok(seen.some((s) => s.endsWith("saved card.png attachment://card.png")), `saved:\n${seen.join("\n")}\n${err}`);
  assert.ok(seen.some((s) => s.startsWith("warn:") && s.includes("Missing was left out")), "a picture that can't be loaded is left out, with a warning");
  const logged = seen.join("\n");
  assert.ok(seen.some((s) => s.startsWith("warn:") && s.includes("Outside was left out") && s.includes("files folder")), `a path from \${…} outside data/files is refused:\n${logged}`);
  assert.ok(!seen.some((s) => s.includes("Kept was left out")), `a file in data/files loads:\n${logged}`);
  // The fonts were written once into the data folder.
  const fonts = fs.readdirSync(path.join(dir, "data", ".assets", "core.build_canvas")).sort();
  assert.deepStrictEqual(fonts, ["Hack-Regular.ttf", "NotoEmoji-Regular.ttf", "Ubuntu-Bold.ttf", "Ubuntu-Light.ttf"]);

  const img = await napi.loadImage(fs.readFileSync(out));
  assert.deepStrictEqual([img.width, img.height], [400, 200]);
  const c = napi.createCanvas(400, 200);
  const g = c.getContext("2d");
  g.drawImage(img, 0, 0);
  const at = (x, y) => [...g.getImageData(x, y, 1, 1).data.slice(0, 3)];
  assert.deepStrictEqual(at(100, 100), [255, 0, 0], "the box");
  assert.deepStrictEqual(at(300, 100), [0, 0, 255], "the dot, its colour from ${dot}");
  assert.deepStrictEqual(at(380, 20), [16, 32, 48], "the background; the hidden layer isn't drawn");
  assert.deepStrictEqual(at(50, 190), [0, 255, 0], "the bar, half full");
  assert.deepStrictEqual(at(150, 190), [0, 0, 0], "the rest of the bar");
  // The name is drawn: some light pixels where it is.
  let light = 0;
  for (let x = 210; x < 390; x += 2) for (let y = 160; y < 186; y += 2) if (at(x, y)[0] > 200) light++;
  assert.ok(light > 10, `the text is drawn (${light} light pixels)`);
  fs.rmSync(dir, { recursive: true, force: true });
  console.log("canvas test passed");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
