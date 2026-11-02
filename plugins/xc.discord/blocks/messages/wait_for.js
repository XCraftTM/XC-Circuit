const KINDS = { message: "A message in a channel", button: "A button or menu on a message", reaction: "A reaction on a message" };

module.exports = {
  id: "discord.wait_for",
  name: "Wait for Reply",
  description: "Waits for someone to answer: a message, a button press or a reaction. Continues on Received, or on Timed out.",
  author: "XC Team",
  version: "1.0.0",
  category: "messages",
  icon: "clock",
  kind: "action",
  flows: ["received", "timedOut"],
  tags: ["collector", "await", "answer", "confirm", "question"],
  fields: {
    kind: { type: "select", pin: false, default: "message", label: "Wait for", choices: KINDS, description: "What counts as an answer." },
    channel: { type: "discord.channel", pin: "only", required: true, show: { kind: ["message"] }, description: "Where to listen." },
    message: { type: "discord.message", pin: "only", required: true, show: { kind: ["button", "reaction"] }, description: "The message with the buttons, or to react to." },
    user: { type: "discord.user?", pin: "only", label: "Only from", description: "Only this person's answer counts. Empty = anyone (but bots)." },
    contains: { type: "text", default: "", show: { kind: ["message"] }, description: "Only messages containing this text." },
    emoji: { type: "text", default: "", show: { kind: ["reaction"] }, description: "Only this emoji. Empty = any." },
    seconds: { type: "number", default: 60, min: 1, max: 3600, label: "Timeout (s)", description: "How long to wait." },
  },
  outputs: {
    message: { type: "discord.message?", description: "The answer message (for messages)." },
    content: { type: "text", description: "Its text (for messages)." },
    interaction: { type: "discord.interaction?", description: "The button or menu press (answer it with Send Message › Update)." },
    customId: { type: "text", label: "Custom id", description: "The pressed button's or menu's id." },
    emoji: { type: "text", description: "The reaction's emoji." },
    user: { type: "discord.user?", description: "Who answered." },
  },
  async run({ fields: f, ctx }) {
    const time = Math.max(1, Math.min(3600, Number(f.seconds) || 60)) * 1000;
    const who = f.user?.id;
    const okUser = (u) => (who ? u.id === who : !u.bot);
    try {
      if (f.kind === "button") {
        const i = await f.message.awaitMessageComponent({ time, filter: (i) => okUser(i.user) });
        await ctx.next("received", { interaction: i, customId: i.customId, user: i.user, content: "", emoji: "", message: null });
      } else if (f.kind === "reaction") {
        const want = String(f.emoji || "");
        const got = await f.message.awaitReactions({ max: 1, time, errors: ["time"], filter: (r, u) => okUser(u) && (!want || r.emoji.name === want || r.emoji.toString() === want) });
        const r = got.first();
        const u = r.users.cache.filter(okUser).last();
        await ctx.next("received", { emoji: r.emoji.toString(), user: u ?? null, interaction: null, customId: "", content: "", message: null });
      } else {
        const text = String(f.contains || "").toLowerCase();
        const got = await f.channel.awaitMessages({ max: 1, time, errors: ["time"], filter: (m) => okUser(m.author) && (!text || m.content.toLowerCase().includes(text)) });
        const m = got.first();
        await ctx.next("received", { message: m, content: m.content, user: m.author, interaction: null, customId: "", emoji: "" });
      }
    } catch (e) {
      // Collectors end with "time" when nothing came.
      const timedOut = e?.code === "InteractionCollectorError" || /time/i.test(String(e?.message ?? e)) || (e && typeof e.size === "number");
      if (!timedOut) throw e;
      await ctx.next("timedOut");
    }
  },
};
