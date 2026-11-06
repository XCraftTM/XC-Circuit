// Captcha for Fluxer (no buttons yet): the bot posts a picture with a code,
// or a sum, in a channel and waits for the person's answer there.

const MODES = { image: "Type the code from a picture", math: "Solve a sum" };
const CODE_CHARS = "ACDEFHJKMNPRTUVWXY2346789";
const pick = (list) => list[Math.floor(Math.random() * list.length)];

let fontReady = false;

function codePicture(ctx, code) {
  const napi = ctx.require("@napi-rs/canvas");
  if (!fontReady) {
    napi.GlobalFonts.registerFromPath(ctx.asset("../../fonts/Ubuntu-Bold.ttf"), "XC Captcha");
    fontReady = true;
  }
  const w = 60 + code.length * 46;
  const h = 110;
  const canvas = napi.createCanvas(w, h);
  const g = canvas.getContext("2d");
  g.fillStyle = "#2B2D31";
  g.fillRect(0, 0, w, h);
  const r = (a, b) => a + Math.random() * (b - a);
  const colour = () => `hsl(${Math.floor(r(0, 360))}, 70%, ${Math.floor(r(60, 78))}%)`;
  for (let i = 0; i < 6; i++) {
    g.strokeStyle = colour();
    g.globalAlpha = 0.35;
    g.lineWidth = r(1, 3);
    g.beginPath();
    g.moveTo(r(0, w), r(0, h));
    g.bezierCurveTo(r(0, w), r(0, h), r(0, w), r(0, h), r(0, w), r(0, h));
    g.stroke();
  }
  g.globalAlpha = 1;
  g.textBaseline = "middle";
  [...code].forEach((c, i) => {
    g.save();
    g.translate(32 + i * 46 + r(-4, 4), h / 2 + r(-10, 10));
    g.rotate(r(-0.45, 0.45));
    g.font = `${Math.floor(r(46, 58))}px "XC Captcha"`;
    g.fillStyle = colour();
    g.fillText(c, 0, 0);
    g.restore();
  });
  g.strokeStyle = colour();
  g.lineWidth = 3;
  g.beginPath();
  g.moveTo(10, r(30, 80));
  g.bezierCurveTo(w / 3, r(0, h), (2 * w) / 3, r(0, h), w - 10, r(30, 80));
  g.stroke();
  for (let i = 0; i < 120; i++) {
    g.fillStyle = colour();
    g.fillRect(r(0, w), r(0, h), 2, 2);
  }
  return canvas.toBuffer("image/png");
}

function challenge(ctx, mode, length) {
  if (mode === "math") {
    const op = pick(["+", "-", "×"]);
    let [a, b] = [Math.floor(Math.random() * 11) + 2, Math.floor(Math.random() * 11) + 2];
    if (op === "-" && b > a) [a, b] = [b, a];
    const result = op === "+" ? a + b : op === "-" ? a - b : a * b;
    return { text: `What is **${a} ${op} ${b}**? Write the result here.`, ok: (x) => Number(String(x).trim()) === result };
  }
  const code = Array.from({ length: Math.min(8, Math.max(4, Number(length) || 5)) }, () => pick(CODE_CHARS)).join("");
  return { text: "Write the code in the picture here (upper or lower case, it doesn't matter).", picture: codePicture(ctx, code), ok: (a) => String(a).replace(/\s/g, "").toUpperCase() === code };
}

module.exports = {
  id: "fluxer.captcha",
  kind: "action",
  info: {
    name: "Captcha",
    description: "Checks that someone is a person: the bot posts a code in a picture, or a sum, in a channel, and waits for their answer there. Then Passed, Failed or Timed Out.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["Verify", "Verification", "Human check", "Anti-bot"],
    tags: ["captcha", "verify", "verification", "human", "bot", "raid", "spam"],
  },
  look: {
    icon: "shield",
  },
  requires: {
    dependencies: { "@napi-rs/canvas": "^1.1.0" },
    assets: ["../../fonts/Ubuntu-Bold.ttf"],
  },
  flows: ["passed", "failed", "timedOut"],
  fields: {
    mode: { type: "select", pin: false, default: "image", choices: MODES, description: "What they do." },
    channel: { type: "fluxer.channel", pin: "only", required: true, description: "Where to ask, such as a #verify channel." },
    user: { type: "fluxer.user", pin: "only", required: true, description: "Who to check (a member works too)." },
    attempts: { type: "number", pin: false, default: 3, min: 1, max: 10, description: "How many tries they get." },
    seconds: { type: "number", pin: false, default: 180, min: 15, max: 900, label: "Time (s)", description: "How long they have, in all." },
    tidy: { type: "boolean", pin: false, default: true, advanced: true, label: "Clean up", description: "Delete the questions and their answers afterwards." },
    length: { type: "number", pin: false, default: 5, min: 4, max: 8, advanced: true, label: "Code length", show: { mode: ["image"] } },
  },
  outputs: {
    user: { type: "fluxer.user", description: "Who it was." },
    tries: { type: "number", description: "How many tries they needed (or used)." },
  },
  async run({ fields: f, ctx }) {
    const client = ctx.fluxer.client;
    const user = f.user?.user ?? f.user;
    const userId = ctx.fluxer.id(user, "user");
    const channel = await ctx.fluxer.channel(f.channel);
    const deadline = Date.now() + (Number(f.seconds) || 180) * 1000;
    let left = Math.max(1, Number(f.attempts) || 3);
    let tries = 0;
    const toDelete = [];
    const ask = async (c, note) => {
      const text = [`<@${userId}>`, note ? `❌ ${note}` : "", c.text, `-# ${left} ${left === 1 ? "try" : "tries"} left`].filter(Boolean).join("\n");
      toDelete.push(await channel.send({ content: text, files: c.picture ? [{ name: "captcha.png", data: c.picture }] : undefined }));
    };
    /** Their next message in the channel, or null when time's up. */
    const answer = () =>
      new Promise((resolve) => {
        const handler = (m) => {
          if (String(m.channelId) !== String(channel.id) || String(m.author?.id) !== String(userId)) return;
          done(m);
        };
        const done = (m) => {
          clearTimeout(timer);
          client.off("messageCreate", handler);
          resolve(m);
        };
        const timer = setTimeout(() => done(null), Math.max(0, deadline - Date.now()));
        client.on("messageCreate", handler);
      });
    const finish = async (flow, text) => {
      const last = await channel.send({ content: `<@${userId}> ${text}` }).catch(() => null);
      if (f.tidy !== false) {
        for (const m of toDelete) m?.delete?.().catch(() => {});
        if (last) setTimeout(() => last.delete?.().catch(() => {}), 10000).unref?.();
      }
      return ctx.next(flow, { user, tries });
    };

    let c = challenge(ctx, f.mode, f.length);
    await ask(c);
    for (;;) {
      const m = await answer();
      if (!m) return finish("timedOut", "⌛ Time's up.");
      toDelete.push(m);
      tries++;
      if (c.ok(m.content ?? "")) return finish("passed", "✅ Thanks, that's right.");
      left--;
      if (left <= 0) return finish("failed", "❌ That wasn't right, and there are no tries left.");
      c = challenge(ctx, f.mode, f.length);
      await ask(c, "That wasn't right. Here's a new one.");
    }
  },

  // For tests.
  challenge,
};
