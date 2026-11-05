const KINDS = { message: "A message", reaction: "A reaction" };

module.exports = {
  id: "fluxer.wait_for",
  kind: "action",
  info: {
    name: "Wait For",
    description: "Waits for the next message in a channel, or a reaction on a message, then goes on with it. Timed out runs when nothing came in time. For questions and confirmations.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "messages",
    aliases: ["Await message", "Wait for reply", "Ask question", "Confirm"],
  },
  look: {
    icon: "clock",
  },
  flows: ["received", "timedOut"],
  fields: {
    kind: { type: "select", pin: false, default: "message", label: "Wait for", choices: KINDS, description: "What counts as an answer." },
    channel: { type: "fluxer.channel", pin: "only", required: true, show: { kind: ["message"] }, description: "Where to listen." },
    message: { type: "fluxer.message", pin: "only", required: true, show: { kind: ["reaction"] }, description: "The message to react to." },
    from: { type: "any", pin: true, description: "Only from this person (a user, member or id). Empty = anyone but bots." },
    contains: { type: "text", default: "", show: { kind: ["message"] }, description: "Only messages containing this text." },
    emoji: { type: "text", default: "", show: { kind: ["reaction"] }, description: "Only this emoji. Empty = any." },
    seconds: { type: "number", default: 60, min: 1, max: 900, step: 1, description: "How long to wait (up to 15 minutes)." },
  },
  outputs: {
    answer: { type: "fluxer.message?", description: "The message that came (for messages)." },
    content: { type: "text", description: "Its text, or the emoji of the reaction." },
    user: { type: "fluxer.user?", description: "Who answered." },
  },
  async run({ fields: f, ctx }) {
    const client = ctx.fluxer.client;
    const fromId = f.from ? ctx.fluxer.id(f.from, "user") : null;
    const ms = Math.min(900, Math.max(1, Number(f.seconds) || 60)) * 1000;
    let channelId = null;
    let messageId = null;
    if (f.kind === "reaction") {
      if (!f.message) throw new Error("Connect the message");
      messageId = String(f.message.id);
    } else {
      channelId = String((await ctx.fluxer.channel(f.channel)).id);
    }
    const want = String(f.emoji ?? "").trim();
    const contains = String(f.contains ?? "").toLowerCase();
    const got = await new Promise((resolve) => {
      let timer;
      const done = (value) => {
        clearTimeout(timer);
        client.off(f.kind === "reaction" ? "messageReactionAdd" : "messageCreate", handler);
        resolve(value);
      };
      const handler = async (x) => {
        if (f.kind === "reaction") {
          if (String(x.messageId) !== messageId) return;
          const e = x.emoji || {};
          if (want && want !== e.name && want !== `<:${e.name}:${e.id}>` && want !== String(e.id)) return;
          const user = x.user ?? (await client.users.resolve(x.userId).catch(() => null));
          if (fromId ? String(x.userId) !== fromId : user?.bot) return;
          done({ answer: null, content: e.id ? `<${e.animated ? "a" : ""}:${e.name}:${e.id}>` : e.name, user });
        } else {
          if (String(x.channelId) !== channelId) return;
          if (fromId ? String(x.author?.id) !== fromId : x.author?.bot) return;
          if (contains && !String(x.content ?? "").toLowerCase().includes(contains)) return;
          done({ answer: x, content: x.content ?? "", user: x.author ?? null });
        }
      };
      client.on(f.kind === "reaction" ? "messageReactionAdd" : "messageCreate", handler);
      timer = setTimeout(() => done(null), ms);
    });
    if (!got) {
      await ctx.next("timedOut", { answer: null, content: "", user: null });
      return;
    }
    await ctx.next("received", got);
  },
};
