// Captcha for Discord and Fluxer, with stand-ins that play the person:
// a wrong answer then the right one (Passed), only wrong ones (Failed), no
// answer (Timed Out), clicks from someone else ignored. The picture mode
// needs @napi-rs/canvas in XC_TEST_DEPS; without it, that part is skipped.
// Usage: [XC_TEST_DEPS=<dir>] node runtime/test/captcha.js
"use strict";

const path = require("path");
const Module = require("module");
const assert = require("assert");
const { EventEmitter } = require("events");

const root = path.join(__dirname, "..", "..");
const discordBlock = require(path.join(root, "plugins", "xc.discord", "blocks", "members", "captcha.js"));
const fluxerBlock = require(path.join(root, "plugins", "xc.fluxer", "blocks", "members", "captcha.js"));
const deps = process.env.XC_TEST_DEPS ? Module.createRequire(path.join(path.resolve(process.env.XC_TEST_DEPS), "package.json")) : null;
let napi = null;
try {
  napi = deps && deps("@napi-rs/canvas");
} catch {}

// ── discord.js stand-ins: builders that remember what they were given.
class Builder {
  constructor() {
    this.data = { components: [] };
  }
  addComponents(...c) {
    this.data.components.push(...c);
    return this;
  }
}
for (const n of ["setCustomId", "setLabel", "setStyle", "setEmoji", "setRequired", "setMaxLength", "setTitle"]) {
  Builder.prototype[n] = function (v) {
    this.data[n[3].toLowerCase() + n.slice(4)] = v;
    return this;
  };
}
const fakeDiscord = {
  ActionRowBuilder: Builder,
  ButtonBuilder: Builder,
  ModalBuilder: Builder,
  TextInputBuilder: Builder,
  ButtonStyle: { Primary: 1, Secondary: 2, Success: 3 },
  TextInputStyle: { Short: 1 },
  MessageFlags: { Ephemeral: 64 },
  AttachmentBuilder: class {
    constructor(data, o) {
      Object.assign(this, { data, ...o });
    }
  },
};

const EMOJI = { apple: "🍎", dog: "🐶", car: "🚗", moon: "🌙", star: "⭐", balloon: "🎈", pizza: "🍕", fish: "🐟", cactus: "🌵", football: "⚽", guitar: "🎸", rocket: "🚀" };
/** The right answer to what a message asks. */
function solve(content) {
  const sum = /\*\*(\d+) ([+\-×]) (\d+)\*\*/.exec(content);
  if (sum) return String(sum[2] === "+" ? +sum[1] + +sum[3] : sum[2] === "-" ? sum[1] - sum[3] : sum[1] * sum[3]);
  const emoji = /Click the \*\*(\w+)\*\*/.exec(content);
  if (emoji) return EMOJI[emoji[1]];
  return "ok";
}

/** Runs Discord's Captcha with a person who answers `plan` (each "wrong", "right" or "none"). */
async function discordRun(mode, plan, where = "reply") {
  const shown = [];
  const person = { id: "u1", username: "Alex", send: async (p) => (shown.push(p), message) };
  let step = 0;
  let prefix = null;
  const message = {
    id: "m1",
    edit: async (p) => shown.push(p),
    awaitMessageComponent: async ({ filter, time }) => {
      assert.ok(time > 0);
      const what = plan[step++];
      const last = shown[shown.length - 1];
      const row = last.components[0].data.components;
      prefix = row[0].data.customId.split(":").slice(0, 3).join(":");
      // Someone else's click doesn't count.
      assert.strictEqual(filter({ user: { id: "u2" }, customId: row[0].data.customId }), false);
      if (!what || what === "none") throw new Error("time");
      const right = solve(last.content);
      const typed = row[0].data.customId.endsWith(":answer");
      const button = typed ? row[0] : row.find((b) => (what === "right" ? (b.data.emoji ?? "ok") === right || b.data.customId.endsWith(":ok") : (b.data.emoji ?? "ok") !== right)) || row[0];
      const click = {
        user: person,
        customId: button.data.customId,
        update: async (p) => shown.push(p),
        showModal: async (m) => {
          assert.strictEqual(m.data.components[0].data.components[0].data.customId, "answer");
          click.modal = m;
        },
        awaitModalSubmit: async ({ filter: ff }) => {
          const submit = { user: person, customId: click.modal.data.customId, fields: { getTextInputValue: () => (what === "right" ? right : "999999") }, update: async (p) => shown.push(p) };
          assert.ok(ff(submit));
          return submit;
        },
      };
      assert.ok(filter(click), "their own click counts");
      return click;
    },
  };
  const interaction = {
    user: person,
    deferred: false,
    replied: false,
    reply: async (p) => {
      assert.strictEqual(p.flags, 64, "only they see it");
      assert.strictEqual(p.withResponse, true);
      shown.push(p);
      return { resource: { message } };
    },
    webhook: { editMessage: async (id, p) => (assert.strictEqual(id, "m1"), shown.push(p)) },
  };
  let result = null;
  const ctx = { require: (n) => (n === "discord.js" ? fakeDiscord : deps(n)), asset: (p) => path.resolve(root, "plugins", "xc.discord", "blocks", "members", p), next: async (flow, out) => (result = { flow, ...out }) };
  await discordBlock.run({ fields: { mode, where, interaction, user: person, title: "Check", attempts: 2, seconds: 60, length: 5 }, ctx });
  return { result, shown, prefix };
}

(async () => {
  for (const mode of ["math", "emoji", "button"]) {
    const ok = await discordRun(mode, ["wrong", "right"]);
    const expected = mode === "button" ? 1 : 2; // the button is always right
    assert.deepStrictEqual([ok.result.flow, ok.result.tries], ["passed", expected], `${mode}: passed`);
    assert.strictEqual(ok.shown[ok.shown.length - 1].content, "✅ Thanks, that's right.");
    if (mode !== "button") {
      assert.ok(ok.shown[1].content.includes("That wasn't right"), `${mode}: a new challenge after a wrong answer`);
      const bad = await discordRun(mode, ["wrong", "wrong"]);
      assert.deepStrictEqual([bad.result.flow, bad.result.tries], ["failed", 2], `${mode}: failed`);
      assert.deepStrictEqual(bad.shown[bad.shown.length - 1].components, []);
    }
  }
  const late = await discordRun("math", ["none"]);
  assert.strictEqual(late.result.flow, "timedOut");
  assert.strictEqual(late.shown[late.shown.length - 1].content, "⌛ Time's up.", "changed through the interaction");
  const dm = await discordRun("emoji", ["right"], "dm");
  assert.strictEqual(dm.result.flow, "passed", "by direct message");

  // Fluxer: questions in a channel, answers as messages.
  const fluxer = async (mode, answers) => {
    const client = new EventEmitter();
    client.off = client.removeListener;
    const sent = [];
    const deleted = [];
    let i = 0;
    const channel = {
      id: "c1",
      send: async (p) => {
        sent.push(p);
        const m = { content: p.content, delete: async () => deleted.push(p.content) };
        // The person answers the question (not the bot's last word).
        if (/try|tries/.test(p.content) && i < answers.length) {
          const what = answers[i++];
          setTimeout(() => {
            client.emit("messageCreate", { channelId: "c1", author: { id: "u9" }, content: "not me" });
            if (what !== "none") client.emit("messageCreate", { channelId: "c1", author: { id: "u1" }, content: what === "right" ? solve(p.content) : "nope", delete: async () => deleted.push("answer") });
          }, 5);
        }
        return m;
      },
    };
    let result = null;
    const ctx = {
      fluxer: { client, id: (u) => u.id, channel: async () => channel },
      require: (n) => deps(n),
      asset: (p) => path.resolve(root, "plugins", "xc.fluxer", "blocks", "members", p),
      next: async (flow, out) => (result = { flow, ...out }),
    };
    await fluxerBlock.run({ fields: { mode, channel: "c1", user: { id: "u1" }, attempts: 2, seconds: answers.includes("none") ? 0.2 : 30, tidy: true }, ctx });
    return { result, sent, deleted };
  };
  const fx = await fluxer("math", ["wrong", "right"]);
  assert.deepStrictEqual([fx.result.flow, fx.result.tries], ["passed", 2]);
  assert.ok(fx.sent[0].content.startsWith("<@u1>"), "it mentions them");
  assert.ok(fx.deleted.includes("answer"), "answers are cleaned up");
  assert.strictEqual((await fluxer("math", ["wrong", "wrong"])).result.flow, "failed");
  assert.strictEqual((await fluxer("math", ["none"])).result.flow, "timedOut");

  // The picture: a PNG with the code, which only the right code answers.
  if (napi) {
    const ctx = { require: (n) => deps(n), asset: (p) => path.resolve(root, "plugins", "xc.discord", "blocks", "members", p) };
    const c = discordBlock.challenge(ctx, "image", 6);
    const img = await napi.loadImage(c.picture);
    assert.deepStrictEqual([img.width, img.height], [60 + 6 * 46, 110]);
    assert.ok(c.typed && !c.ok("AAAAAA") && !c.ok(""));
    const f = fluxerBlock.challenge({ require: (n) => deps(n), asset: (p) => path.resolve(root, "plugins", "xc.fluxer", "blocks", "members", p) }, "image", 5);
    assert.ok(Buffer.isBuffer(f.picture) && f.picture.subarray(1, 4).toString() === "PNG");
  } else {
    console.log("  (the picture skipped: @napi-rs/canvas isn't in XC_TEST_DEPS)");
  }
  console.log("captcha test passed");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
