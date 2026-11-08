// Build Canvas: draws a picture (a welcome card, a rank card …) from a design
// made in the Canvas Builder, with @napi-rs/canvas. The fonts are the ones
// the editor uses, so the picture looks like the Canvas Builder's preview.

const FONT_FILES = {
  "XC Ubuntu": "../../fonts/Ubuntu-Light.ttf",
  "XC Ubuntu Bold": "../../fonts/Ubuntu-Bold.ttf",
  "XC Hack": "../../fonts/Hack-Regular.ttf",
  "XC Emoji": "../../fonts/NotoEmoji-Regular.ttf",
};
const FAMILY = { regular: "XC Ubuntu", bold: "XC Ubuntu Bold", mono: "XC Hack" };

let fontsReady = false;

/** Registers the fonts once per bot. */
function registerFonts(ctx, GlobalFonts) {
  if (fontsReady) return;
  for (const [family, file] of Object.entries(FONT_FILES)) GlobalFonts.registerFromPath(ctx.asset(file), family);
  fontsReady = true;
}

const num = (v, fallback = 0) => (Number.isFinite(Number(v)) ? Number(v) : fallback);

/** A colour as the canvas takes it: #RRGGBB(AA), a CSS colour, or a number (a role's colour). */
function colour(v, fallback = "#000000") {
  if (typeof v === "number") return `#${(v & 0xffffff).toString(16).padStart(6, "0")}`;
  const s = String(v ?? "").trim();
  if (!s) return fallback;
  // Hex, with or without its # (FF5CA8 from a variable or an input).
  if (/^#?([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(s)) return s.startsWith("#") ? s : `#${s}`;
  if (/^\d+$/.test(s)) return `#${(Number(s) & 0xffffff).toString(16).padStart(6, "0")}`;
  return s;
}

/** The fill of a layer: its colour, or a gradient to `color2` at `angle` degrees. */
function paintOf(g, layer, x, y, w, h, fill) {
  const c1 = colour(fill(layer.color), "#FFFFFF");
  if (!layer.color2) return c1;
  const a = (num(layer.angle) * Math.PI) / 180;
  const cx = x + w / 2;
  const cy = y + h / 2;
  const r = (Math.abs(w * Math.cos(a)) + Math.abs(h * Math.sin(a))) / 2;
  const grad = g.createLinearGradient(cx - Math.cos(a) * r, cy - Math.sin(a) * r, cx + Math.cos(a) * r, cy + Math.sin(a) * r);
  grad.addColorStop(0, c1);
  grad.addColorStop(1, colour(fill(layer.color2), c1));
  return grad;
}

/** The outline of a shape as the current path: a rectangle with round corners, or an ellipse. */
function shapePath(g, shape, x, y, w, h, radius) {
  g.beginPath();
  if (shape === "circle") g.ellipse(x + w / 2, y + h / 2, Math.abs(w / 2), Math.abs(h / 2), 0, 0, Math.PI * 2);
  else g.roundRect(x, y, w, h, Math.max(0, Math.min(num(radius), Math.abs(w) / 2, Math.abs(h) / 2)));
}

function outline(g, layer, fill) {
  const width = num(layer.outlineWidth);
  if (!layer.outline || width <= 0) return;
  g.lineWidth = width;
  g.strokeStyle = colour(fill(layer.outline));
  g.stroke();
}

/** The most a picture from the web may be. */
const PICTURE_MAX = 20 * 1024 * 1024;

/**
 * A picture from a web address, a data: URL, or a file path. When the design
 * fills it from ${…} (`fromPeople`), whoever typed the value decides it: then
 * a web address must be public, and a file must be in the bot's files folder.
 */
async function picture(src, loadImage, { fromPeople = false, dataDir, fetch: get } = {}) {
  if (/^https?:\/\//i.test(src)) {
    if (get) {
      const { res, body } = await get(src, { signal: AbortSignal.timeout(10000) }, { publicOnly: fromPeople, maxBytes: PICTURE_MAX });
      if (!res.ok) throw new Error(`${src} answered ${res.status}`);
      return loadImage(body);
    }
    const res = await fetch(src, { signal: AbortSignal.timeout(10000) });
    if (!res.ok) throw new Error(`${src} answered ${res.status}`);
    return loadImage(Buffer.from(await res.arrayBuffer()));
  }
  if (/^data:/i.test(src)) return loadImage(src);
  const fs = require("fs");
  const path = require("path");
  if (fromPeople) {
    const root = path.join(dataDir || process.cwd(), "files");
    const full = path.resolve(root, src);
    if (!full.startsWith(root + path.sep)) throw new Error("A picture filled in from ${…} is a web address or a file in the bot's files folder (data/files), by its name");
    return loadImage(await fs.promises.readFile(full));
  }
  // A file: read here, as loadImage takes C:\… for a web address.
  return loadImage(await fs.promises.readFile(src));
}

/** Lines of `text` that fit in `width` (0: no wrapping). */
function wrap(g, text, width) {
  const out = [];
  for (const para of String(text).split(/\r?\n/)) {
    if (!(width > 0)) {
      out.push(para);
      continue;
    }
    let line = "";
    for (const word of para.split(/(\s+)/)) {
      const next = line + word;
      if (line && g.measureText(next.trimEnd()).width > width) {
        out.push(line.trimEnd());
        line = word.trimStart();
      } else line = next;
    }
    out.push(line.trimEnd());
  }
  return out;
}

const LAYERS = {
  rect(g, l, fill) {
    shapePath(g, "rect", l.x, l.y, l.w, l.h, l.radius);
    g.fillStyle = paintOf(g, l, l.x, l.y, l.w, l.h, fill);
    g.fill();
    outline(g, l, fill);
  },
  circle(g, l, fill) {
    shapePath(g, "circle", l.x, l.y, l.w, l.h);
    g.fillStyle = paintOf(g, l, l.x, l.y, l.w, l.h, fill);
    g.fill();
    outline(g, l, fill);
  },
  async image(g, l, fill, { loadImage }, pictures = {}) {
    const src = String(fill(l.src) ?? "").trim();
    if (!src) return;
    const img = await picture(src, loadImage, { ...pictures, fromPeople: String(l.src ?? "").includes("${") });
    const shape = l.shape === "circle" ? "circle" : "rect";
    const radius = l.shape === "rounded" ? num(l.radius, 16) : 0;
    g.save();
    shapePath(g, shape, l.x, l.y, l.w, l.h, radius);
    g.clip();
    // cover fills the box (cutting off what sticks out), contain shows all of it, stretch fills it exactly.
    let [sx, sy, sw, sh] = [0, 0, img.width, img.height];
    let [dx, dy, dw, dh] = [l.x, l.y, l.w, l.h];
    if (l.fit === "contain") {
      const s = Math.min(l.w / img.width, l.h / img.height);
      [dw, dh] = [img.width * s, img.height * s];
      [dx, dy] = [l.x + (l.w - dw) / 2, l.y + (l.h - dh) / 2];
    } else if (l.fit !== "stretch") {
      const s = Math.max(l.w / img.width, l.h / img.height);
      [sw, sh] = [l.w / s, l.h / s];
      [sx, sy] = [(img.width - sw) / 2, (img.height - sh) / 2];
    }
    g.drawImage(img, sx, sy, sw, sh, dx, dy, dw, dh);
    g.restore();
    shapePath(g, shape, l.x, l.y, l.w, l.h, radius);
    outline(g, l, fill);
  },
  text(g, l, fill) {
    const text = String(fill(l.text) ?? "");
    if (!text) return;
    const family = `"${FAMILY[l.font] || FAMILY.regular}", "XC Emoji"`;
    let size = Math.max(4, num(l.size, 32));
    const width = num(l.w);
    g.font = `${size}px ${family}`;
    // Shrink to fit: one line, smaller until it fits the box's width.
    if (l.shrink && width > 0 && !text.includes("\n")) {
      while (size > 8 && g.measureText(text).width > width) {
        size -= 1;
        g.font = `${size}px ${family}`;
      }
    }
    const lines = l.shrink ? [text] : wrap(g, text, width);
    const lineHeight = size * 1.25;
    const align = ["center", "right"].includes(l.align) ? l.align : "left";
    const x = align === "center" ? l.x + width / 2 : align === "right" ? l.x + width : l.x;
    g.textAlign = align;
    g.textBaseline = "top";
    lines.forEach((line, i) => {
      const y = l.y + i * lineHeight;
      if (l.outline && num(l.outlineWidth) > 0) {
        g.lineJoin = "round";
        g.lineWidth = num(l.outlineWidth) * 2;
        g.strokeStyle = colour(fill(l.outline));
        g.strokeText(line, x, y);
      }
      if (l.shadow) {
        g.shadowColor = "rgba(0, 0, 0, 0.6)";
        g.shadowBlur = size * 0.15;
        g.shadowOffsetY = Math.max(1, size * 0.06);
      }
      g.fillStyle = paintOf(g, l, l.x, l.y, width || size * line.length * 0.6, lineHeight * lines.length, fill);
      g.fillText(line, x, y);
      g.shadowColor = "transparent";
    });
  },
  bar(g, l, fill) {
    const max = num(fill(l.max), 100) || 100;
    const part = Math.max(0, Math.min(1, num(fill(l.value)) / max));
    shapePath(g, "rect", l.x, l.y, l.w, l.h, l.radius);
    g.fillStyle = colour(fill(l.back), "#484B51");
    g.fill();
    g.save();
    shapePath(g, "rect", l.x, l.y, l.w, l.h, l.radius);
    g.clip();
    g.fillStyle = paintOf(g, l, l.x, l.y, l.w * part, l.h, fill);
    g.fillRect(l.x, l.y, l.w * part, l.h);
    g.restore();
    shapePath(g, "rect", l.x, l.y, l.w, l.h, l.radius);
    outline(g, l, fill);
  },
};

/** Draws `design` and gives the PNG. Layers that fail (a picture that can't be loaded) are left out, with a warning. */
/** `pictures`: { dataDir, fetch } for pictures filled in from ${…} (see picture). */
async function draw(design, fill, napi, warn, pictures = {}) {
  const width = Math.round(Math.max(16, Math.min(4096, num(design.width, 900))));
  const height = Math.round(Math.max(16, Math.min(4096, num(design.height, 300))));
  const canvas = napi.createCanvas(width, height);
  const g = canvas.getContext("2d");
  if (design.background) {
    g.fillStyle = colour(fill(design.background));
    g.fillRect(0, 0, width, height);
  }
  for (const raw of design.layers || []) {
    if (!raw || raw.hidden || !LAYERS[raw.type]) continue;
    const l = { ...raw, x: num(raw.x), y: num(raw.y), w: num(raw.w), h: num(raw.h) };
    g.save();
    g.globalAlpha = Math.max(0, Math.min(1, num(raw.opacity, 100) / 100));
    try {
      await LAYERS[raw.type](g, l, fill, napi, pictures);
    } catch (e) {
      warn(`${raw.name || raw.type} was left out: ${e.message}`);
    }
    g.restore();
  }
  return canvas.encode("png");
}

module.exports = {
  id: "core.build_canvas",
  kind: "action",
  info: {
    name: "Build Canvas",
    description: "Draws a picture, such as a welcome or rank card, from a design made in the Canvas Builder. ${…} in its texts and pictures holds JavaScript. Send it with Send Message's Files.",
    author: "XC Team",
    version: "1.1.1",
  },
  search: {
    category: "design",
    aliases: ["Canvas", "Image", "Welcome card", "Rank card", "Picture", "Draw"],
    tags: ["canvas", "image", "picture", "card", "welcome", "rank", "level", "avatar", "draw", "png"],
  },
  look: {
    icon: "palette",
    preview: "canvas",
  },
  requires: {
    dependencies: { "@napi-rs/canvas": "^1.1.0" },
    assets: Object.values(FONT_FILES),
  },
  fields: {
    canvas: {
      type: "canvas",
      pin: false,
      // Names used in ${…} in the design get an input, unless earlier blocks provide them.
      template: true,
      label: "Canvas",
      default: {
        width: 934,
        height: 282,
        background: "#1E1F22",
        layers: [
          { type: "rect", name: "Card", x: 16, y: 16, w: 902, h: 250, radius: 24, color: "#2B2D31" },
          { type: "image", name: "Avatar", x: 48, y: 51, w: 180, h: 180, shape: "circle", fit: "cover", src: "${user.displayAvatarURL({ extension: 'png', size: 256 })}", outline: "#00D8F0", outlineWidth: 6 },
          { type: "text", name: "Name", x: 264, y: 64, w: 620, text: "${user.username}", font: "bold", size: 46, color: "#FFFFFF", shrink: true, shadow: true },
          { type: "text", name: "Level", x: 264, y: 128, w: 620, text: "Level ${level} · ${xp} / ${needed} XP", size: 28, color: "#B5BAC1" },
          { type: "bar", name: "Progress", x: 264, y: 184, w: 620, h: 34, radius: 17, value: "${xp}", max: "${needed}", color: "#00D8F0", color2: "#3DFF6E", back: "#3F4147" },
        ],
        samples: { "user.username": "Alex", level: "12", xp: "750", needed: "1000" },
      },
      description: "The design. Open the Canvas Builder to edit it.",
    },
    name: { type: "text", default: "canvas.png", advanced: true, label: "File name", description: "The picture's file name, ending in .png." },
  },
  outputs: {
    file: { type: "any", description: "The picture. Connect it to Send Message's Files." },
    url: { type: "text", description: "attachment://canvas.png: put it in an embed's image (and send the file with it) to show the picture inside the embed." },
  },

  async run({ fields, ctx }) {
    const napi = ctx.require("@napi-rs/canvas");
    registerFonts(ctx, napi.GlobalFonts);
    const fill = (v) => (typeof v === "string" && v.includes("${") ? ctx.fill(v, fields) : v);
    const name = String(fields.name || "canvas.png").replace(/[\\/:*?"<>|]/g, "_").replace(/(\.png)?$/i, ".png");
    const png = await draw(fields.canvas || {}, fill, napi, (text) => ctx.log.warn(text), { dataDir: ctx.runtime.dataDir, fetch: ctx.util.fetch });
    return { file: { attachment: png, name }, url: `attachment://${name}` };
  },

  // For tests.
  draw,
};
