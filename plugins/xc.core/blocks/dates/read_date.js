// Turns what people type into a date: 2026-12-24 18:00, 24.12.2026,
// 12/24/2026, 18:30, tomorrow 9:00, in 2 hours, 1h 30m, a unix time or a
// Discord timestamp. Times without a date are today (or tomorrow, once
// passed); dates without a time are at midnight.

const UNIT_MS = { s: 1000, sec: 1000, second: 1000, seconds: 1000, m: 60000, min: 60000, mins: 60000, minute: 60000, minutes: 60000, h: 3600000, hr: 3600000, hrs: 3600000, hour: 3600000, hours: 3600000, d: 86400000, day: 86400000, days: 86400000, w: 604800000, week: 604800000, weeks: 604800000 };
const ORDERS = { dmy: "Day first (24.12.2026, 24/12/2026)", mdy: "Month first (12/24/2026)" };

/** "1h 30m", "2 days": milliseconds, or null when it isn't one. */
function span(s) {
  let total = 0;
  let rest = s;
  for (const m of s.matchAll(/(\d+(?:\.\d+)?)\s*([a-z]+)/g)) {
    const unit = UNIT_MS[m[2]];
    if (!unit) return null;
    total += Number(m[1]) * unit;
    rest = rest.replace(m[0], "");
  }
  return total > 0 && rest.trim() === "" ? total : null;
}

/** "18:30", "9", "6pm", "6:30 pm": [hours, minutes], or null. */
function clock(s) {
  const m = /^(\d{1,2})(?::(\d{2}))?(?::(\d{2}))?\s*(am|pm)?$/.exec(s.trim());
  if (!m || (m[2] === undefined && !m[4])) return null;
  let h = Number(m[1]);
  const min = Number(m[2] || 0);
  if (m[4] === "pm" && h < 12) h += 12;
  if (m[4] === "am" && h === 12) h = 0;
  return h < 24 && min < 60 ? [h, min, Number(m[3] || 0)] : null;
}

/** A date from what was typed; null when it can't be read. `now` is for tests. */
function read(input, order = "dmy", now = new Date()) {
  if (input instanceof Date) return Number.isNaN(input.getTime()) ? null : input;
  if (typeof input === "number") return new Date(input < 1e11 ? input * 1000 : input);
  let s = String(input ?? "").trim().toLowerCase();
  if (!s) return null;
  const discord = /^<t:(-?\d+)(?::[a-z])?>$/i.exec(s);
  if (discord) return new Date(Number(discord[1]) * 1000);
  if (/^-?\d{9,13}$/.test(s)) return new Date(s.length > 11 ? Number(s) : Number(s) * 1000);
  if (s === "now") return new Date(now);
  const inMatch = /^in\s+(.+)$/.exec(s);
  const later = span(inMatch ? inMatch[1] : s);
  if (later !== null) return new Date(now.getTime() + later);
  const agoMatch = /^(.+)\s+ago$/.exec(s);
  if (agoMatch && span(agoMatch[1]) !== null) return new Date(now.getTime() - span(agoMatch[1]));
  // A day word, then maybe a time: today 18:00, tomorrow at 9am.
  const day = /^(today|tomorrow|yesterday)(?:\s+(?:at\s+)?(.+))?$/.exec(s);
  if (day) {
    const d = new Date(now);
    d.setDate(d.getDate() + { today: 0, tomorrow: 1, yesterday: -1 }[day[1]]);
    const t = day[2] ? clock(day[2]) : [0, 0, 0];
    if (!t) return null;
    d.setHours(t[0], t[1], t[2], 0);
    return d;
  }
  // A time alone: today, or tomorrow when it has passed.
  const t = clock(s);
  if (t) {
    const d = new Date(now);
    d.setHours(t[0], t[1], t[2], 0);
    if (d < now) d.setDate(d.getDate() + 1);
    return d;
  }
  // ISO with its time zone (2026-12-24T18:00:00Z, …+01:00): it says when exactly.
  if (/^\d{4}-\d{2}-\d{2}t\d{2}:\d{2}(:\d{2}(\.\d+)?)?(z|[+-]\d{2}:?\d{2})$/.test(s)) {
    const iso = new Date(String(input).trim());
    return Number.isNaN(iso.getTime()) ? null : iso;
  }
  // A date, then maybe a time.
  s = s.replace(/t(\d)/, " $1").replace(/\s+at\s+/, " ");
  const [datePart, ...timeParts] = s.split(/\s+/);
  const time = timeParts.length ? clock(timeParts.join(" ")) : [0, 0, 0];
  if (!time) return null;
  let y, mo, d;
  let m;
  if ((m = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(datePart))) [y, mo, d] = [m[1], m[2], m[3]];
  else if ((m = /^(\d{1,2})[./-](\d{1,2})(?:[./-](\d{2,4}))?$/.exec(datePart))) {
    [d, mo] = order === "mdy" && !datePart.includes(".") ? [m[2], m[1]] : [m[1], m[2]];
    y = m[3] ? (m[3].length === 2 ? "20" + m[3] : m[3]) : String(now.getFullYear());
  } else {
    // Anything else JavaScript understands (with a time zone, such as ISO with Z).
    const any = new Date(String(input).trim());
    return Number.isNaN(any.getTime()) ? null : any;
  }
  const out = new Date(Number(y), Number(mo) - 1, Number(d), time[0], time[1], time[2], 0);
  return out.getMonth() === Number(mo) - 1 && out.getDate() === Number(d) ? out : null;
}

module.exports = {
  id: "core.read_date",
  kind: "value",
  info: {
    name: "Read Date",
    description: "Turns typed text into a date: 2026-12-24 18:00, 24.12.2026, 18:30, tomorrow 9:00, in 2h, 1d 12h, a unix time or a Discord timestamp. For reminders, events and giveaways.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "time",
    aliases: ["Parse Date", "Text to Date", "When"],
    tags: ["date", "time", "parse", "reminder", "tomorrow", "in 2 hours"],
  },
  look: {
    icon: "clock",
  },
  pure: false,
  fields: {
    text: { type: "any", pin: true, default: "", description: "What was typed, such as tomorrow 18:00 or 24.12.2026 or in 2h." },
    order: { type: "select", pin: false, default: "dmy", choices: ORDERS, label: "Dates like 01/02", description: "Whether 01/02 is 1 February (day first) or January 2 (month first). Dates with dots are always day first." },
  },
  outputs: {
    date: { type: "date?", description: "The date; empty when the text isn't one." },
    valid: { type: "boolean", description: "Whether the text could be read as a date." },
    unix: { type: "number", description: "Unix seconds; 0 when it isn't a date." },
  },
  run({ fields: f }) {
    const d = read(f.text, f.order || "dmy");
    return { date: d, valid: d !== null, unix: d ? Math.floor(d.getTime() / 1000) : 0 };
  },
};
