// Cooldowns kept in memory (unless "Survives restarts" is on): name:who -> time it ends.
const ENDS = new Map();

module.exports = {
  id: "core.cooldown",
  kind: "action",
  info: {
    name: "Cooldown",
    description: "Lets something happen only once per so many seconds, per person (or channel, or for everyone). Ready starts the wait; Waiting runs while it lasts.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "time",
    aliases: ["Rate limit", "Abklingzeit", "Spam-Schutz"],
    tags: ["rate limit", "spam", "daily", "timeout", "throttle"],
  },
  look: {
    icon: "clock",
  },
  flows: ["ready", "waiting"],
  fields: {
    name: { type: "text", default: "daily", description: "Which cooldown (different commands use different names)." },
    who: { type: "any", pin: "only", description: "Whose cooldown: a user, member, channel or server (or an id). Empty = one for everyone." },
    seconds: { type: "number", default: 60, min: 0, description: "How long to wait between uses." },
    persist: { type: "boolean", default: false, label: "Survives restarts", description: "Keep running cooldowns in stored data (for long ones such as daily rewards)." },
  },
  outputs: {
    secondsLeft: { type: "number", label: "Seconds left", description: "How long until it's ready again (0 when ready)." },
    readyAt: { type: "date", label: "Ready at", description: "When it's ready again." },
  },
  async run({ fields: f, ctx }) {
    const key = `${f.name}:${f.who == null ? "*" : String(f.who.id ?? f.who)}`;
    const store = f.persist ? ctx.store.ns("cooldowns") : null;
    const now = Date.now();
    const ends = store ? await store.get(key, 0) : ENDS.get(key) || 0;
    if (ends > now) {
      await ctx.next("waiting", { secondsLeft: Math.ceil((ends - now) / 1000), readyAt: new Date(ends) });
      return;
    }
    const until = now + Math.max(0, Number(f.seconds) || 0) * 1000;
    if (store) await store.set(key, until);
    else ENDS.set(key, until);
    await ctx.next("ready", { secondsLeft: 0, readyAt: new Date(until) });
  },
};
