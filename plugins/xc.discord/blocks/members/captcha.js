// Captcha: checks that someone is a person before they get in (a role, a
// channel). They answer in a message only they see (or a DM): type the code
// from a picture, solve a sum, click the emoji that's named, or just click.

const MODES = { image: "Type the code from a picture", math: "Solve a sum", emoji: "Click the named emoji", button: "Just click a button" };
const WHERE = { reply: "To the interaction (only they see it)", dm: "Direct message" };
const EMOJI = [
  ["🍎", "apple"],
  ["🐶", "dog"],
  ["🚗", "car"],
  ["🌙", "moon"],
  ["⭐", "star"],
  ["🎈", "balloon"],
  ["🍕", "pizza"],
  ["🐟", "fish"],
  ["🌵", "cactus"],
  ["⚽", "football"],
  ["🎸", "guitar"],
  ["🚀", "rocket"],
];
/** Letters and digits no one mixes up (no 0/O, 1/I/L, 5/S …). */
const CODE_CHARS = "ACDEFHJKMNPRTUVWXY2346789";

const pick = (list) => list[Math.floor(Math.random() * list.length)];
const shuffle = (list) => {
  const out = [...list];
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
};

let fontReady = false;

/** A picture of `code`, its letters turned and moved, with lines and dots over it. */
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
  // A line through all the letters, and some dots.
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

/** A new challenge: what to show, and how to tell the right answer. */
function challenge(ctx, mode, length) {
  switch (mode) {
    case "image": {
      const code = Array.from({ length: Math.min(8, Math.max(4, Number(length) || 5)) }, () => pick(CODE_CHARS)).join("");
      return { text: "Type the code in the picture (upper or lower case, it doesn't matter).", typed: true, label: "The code", picture: codePicture(ctx, code), ok: (a) => String(a).replace(/\s/g, "").toUpperCase() === code };
    }
    case "math": {
      const op = pick(["+", "-", "×"]);
      let [a, b] = [Math.floor(Math.random() * 11) + 2, Math.floor(Math.random() * 11) + 2];
      if (op === "-" && b > a) [a, b] = [b, a];
      const result = op === "+" ? a + b : op === "-" ? a - b : a * b;
      return { text: `What is **${a} ${op} ${b}**?`, typed: true, label: "The result", ok: (x) => Number(String(x).trim()) === result };
    }
    case "emoji": {
      const options = shuffle(EMOJI).slice(0, 5);
      const target = pick(options);
      return { text: `Click the **${target[1]}**.`, buttons: options.map(([e]) => ({ id: e, emoji: e })), ok: (x) => x === target[0] };
    }
    default:
      return { text: "Click the button to show you're a person.", buttons: [{ id: "ok", label: "I'm a person" }], ok: () => true };
  }
}

module.exports = {
  id: "discord.captcha",
  kind: "action",
  info: {
    name: "Captcha",
    description: "Checks that someone is a person: type the code from a picture, solve a sum, click the named emoji or just click. In a message only they see (from a button or command), or a DM. Then Passed, Failed or Timed Out.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["Verify", "Verification", "Human check", "Anti-bot", "Raid protection"],
    tags: ["captcha", "verify", "verification", "human", "bot", "raid", "spam", "gate"],
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
    where: { type: "select", pin: false, default: "reply", choices: WHERE, description: "From a Verify button or a command: to the interaction, which only they see. Otherwise a DM (people can turn DMs off)." },
    interaction: { type: "discord.interaction", pin: "only", required: true, show: { where: ["reply"] }, description: "The button click or command, from On Button or On Slash Command." },
    user: { type: "discord.user", pin: "only", required: true, show: { where: ["dm"] }, description: "Who to check (a member works too), from On Member Join, say." },
    title: { type: "text", default: "Show us you're a person", description: "Above the challenge." },
    attempts: { type: "number", pin: false, default: 3, min: 1, max: 10, description: "How many tries they get." },
    seconds: { type: "number", pin: false, default: 180, min: 15, max: 900, label: "Time (s)", description: "How long they have, in all." },
    length: { type: "number", pin: false, default: 5, min: 4, max: 8, advanced: true, label: "Code length", show: { mode: ["image"] } },
  },
  outputs: {
    user: { type: "discord.user", description: "Who it was." },
    tries: { type: "number", description: "How many tries they needed (or used)." },
  },
  async run({ fields: f, ctx }) {
    const d = ctx.require("discord.js");
    const user = f.where === "dm" ? (f.user?.user ?? f.user) : f.interaction?.user;
    if (!user) throw new Error(f.where === "dm" ? "Connect who to check" : "Connect the interaction (a button click or a command)");
    const prefix = `xc-captcha:${Date.now().toString(36)}:${Math.floor(Math.random() * 1e6)}`;
    const deadline = Date.now() + (Number(f.seconds) || 180) * 1000;
    let left = Math.max(1, Number(f.attempts) || 3);
    let tries = 0;
    let ch = challenge(ctx, f.mode, f.length);

    /** The message for a challenge, with a note (a wrong answer) above it. */
    const view = (c, note) => {
      const row = new d.ActionRowBuilder();
      if (c.typed) row.addComponents(new d.ButtonBuilder().setCustomId(`${prefix}:answer`).setLabel("Answer").setStyle(d.ButtonStyle.Primary));
      for (const b of c.buttons || []) {
        const button = new d.ButtonBuilder().setCustomId(`${prefix}:${b.id}`).setStyle(b.label ? d.ButtonStyle.Success : d.ButtonStyle.Secondary);
        if (b.emoji) button.setEmoji(b.emoji);
        if (b.label) button.setLabel(b.label);
        row.addComponents(button);
      }
      const lines = [`**${f.title || "Show us you're a person"}**`, note ? `❌ ${note}` : "", c.text, `-# ${left} ${left === 1 ? "try" : "tries"} · until <t:${Math.floor(deadline / 1000)}:T>`];
      return { content: lines.filter(Boolean).join("\n"), components: [row], files: c.picture ? [new d.AttachmentBuilder(c.picture, { name: "captcha.png" })] : [], attachments: [] };
    };
    const ended = (text) => ({ content: text, components: [], files: [], attachments: [] });

    let message;
    if (f.where === "dm") {
      message = await user.send(view(ch)).catch(() => null);
      if (!message) throw new Error(`${user.username} doesn't take direct messages: use a Verify button instead`);
    } else {
      const i = f.interaction;
      const payload = { ...view(ch), flags: d.MessageFlags.Ephemeral };
      if (i.deferred || i.replied) message = await i.followUp(payload);
      else message = (await i.reply({ ...payload, withResponse: true })).resource?.message;
    }

    // Messages only they see are changed through the interaction, DMs directly.
    const change = (p) => (f.where === "dm" ? message.edit(p) : f.interaction.webhook.editMessage(message.id, p)).catch(() => {});

    for (;;) {
      const time = deadline - Date.now();
      let click = null;
      if (time > 0) click = await message.awaitMessageComponent({ filter: (x) => x.user.id === user.id && x.customId.startsWith(`${prefix}:`), time }).catch(() => null);
      if (!click) {
        await change(ended("⌛ Time's up."));
        return ctx.next("timedOut", { user, tries });
      }
      let answer;
      let respond;
      if (ch.typed) {
        const input = new d.TextInputBuilder().setCustomId("answer").setLabel(ch.label).setStyle(d.TextInputStyle.Short).setRequired(true).setMaxLength(20);
        await click.showModal(new d.ModalBuilder().setCustomId(`${prefix}:modal`).setTitle(String(f.title || "Captcha").slice(0, 45)).addComponents(new d.ActionRowBuilder().addComponents(input)));
        const submit = await click.awaitModalSubmit({ filter: (x) => x.user.id === user.id && x.customId === `${prefix}:modal`, time: Math.max(1000, deadline - Date.now()) }).catch(() => null);
        if (!submit) {
          await change(ended("⌛ Time's up."));
          return ctx.next("timedOut", { user, tries });
        }
        answer = submit.fields.getTextInputValue("answer");
        respond = (p) => submit.update(p);
      } else {
        answer = click.customId.slice(prefix.length + 1);
        respond = (p) => click.update(p);
      }
      tries++;
      if (ch.ok(answer)) {
        await respond(ended("✅ Thanks, that's right."));
        return ctx.next("passed", { user, tries });
      }
      left--;
      if (left <= 0) {
        await respond(ended("❌ That wasn't right, and there are no tries left."));
        return ctx.next("failed", { user, tries });
      }
      ch = challenge(ctx, f.mode, f.length);
      await respond(view(ch, "That wasn't right. Here's a new one."));
    }
  },

  // For tests.
  challenge,
};
