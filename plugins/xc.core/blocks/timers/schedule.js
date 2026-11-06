const WHEN = { in: "After a while", at: "At a time" };
const UNIT_MS = { ms: 1, s: 1000, sec: 1000, second: 1000, seconds: 1000, m: 60000, min: 60000, mins: 60000, minute: 60000, minutes: 60000, h: 3600000, hr: 3600000, hrs: 3600000, hour: 3600000, hours: 3600000, d: 86400000, day: 86400000, days: 86400000, w: 604800000, week: 604800000, weeks: 604800000 };

/** "10m", "1h 30m", "3 days"; a plain number counts as seconds. */
function duration(text) {
  if (typeof text === "number") return text * 1000;
  const s = String(text ?? "").trim().toLowerCase();
  if (/^\d+(\.\d+)?$/.test(s)) return Number(s) * 1000;
  let total = 0;
  let found = false;
  for (const m of s.matchAll(/(\d+(?:\.\d+)?)\s*([a-z]+)/g)) {
    const unit = UNIT_MS[m[2]];
    if (!unit) throw new Error(`Unknown time unit "${m[2]}" (use s, m, h, d or w)`);
    total += Number(m[1]) * unit;
    found = true;
  }
  if (!found) throw new Error(`"${text}" isn't a time such as 10m, 1h 30m or 3d`);
  return total;
}

/** A date, a timestamp in ms, or text such as 2026-12-24 18:00. */
function moment(v) {
  if (v instanceof Date) return v.getTime();
  if (typeof v === "number" || /^\d{10,}$/.test(String(v ?? "").trim())) return Number(v);
  const t = Date.parse(String(v ?? "").trim());
  if (Number.isNaN(t)) throw new Error(`"${v}" isn't a time such as 2026-12-24 18:00`);
  return t;
}

module.exports = {
  id: "core.schedule",
  kind: "action",
  info: {
    name: "Schedule",
    description: "Runs On Scheduled blocks with the same name once, after a while (10m, 3 days) or at a time, with a value along. Kept with the stored data, so it still runs after a restart.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "time",
    aliases: ["Remind me", "Timer", "Later", "Delay", "Reminder", "Timeout end"],
    tags: ["timer", "schedule", "reminder", "later", "once", "delay", "giveaway", "tempban"],
  },
  look: {
    icon: "clock",
  },
  fields: {
    name: { type: "text", pin: false, required: true, default: "reminder", description: "Which On Scheduled blocks run: the ones with this name." },
    when: { type: "select", pin: false, default: "in", choices: WHEN, description: "After a while from now, or at a set time." },
    wait: { type: "text", default: "10m", show: { when: ["in"] }, description: "How long: 30s, 10m, 1h 30m, 3d, 2w. A plain number is seconds." },
    at: { type: "any", default: "", show: { when: ["at"] }, label: "Time", description: "A date, or text such as 2026-12-24 18:00 (in the bot computer's time zone). A time already past runs at once." },
    data: { type: "any", pin: true, label: "Value", description: "Comes out of On Scheduled: a text, a number, or an object with what it needs ({ user, channel, text }). Users, channels and messages come back as their data (ids, names)." },
    key: { type: "text", default: "", advanced: true, description: "A timer with the same key is replaced, such as remind-${user.id}: one reminder per person." },
  },
  outputs: {
    id: { type: "text", description: "The timer's id, for Cancel Scheduled." },
    due: { type: "date", description: "When it will run." },
    replaced: { type: "boolean", description: "Whether it replaced a timer with the same key." },
  },
  async run({ fields, ctx }) {
    const due = fields.when === "at" ? moment(fields.at) : Date.now() + duration(fields.wait);
    const t = await ctx.timers.schedule({ name: fields.name, due, data: fields.data, key: String(fields.key || "").trim() || undefined });
    return { id: t.id, due: new Date(t.due), replaced: t.replaced };
  },
};
