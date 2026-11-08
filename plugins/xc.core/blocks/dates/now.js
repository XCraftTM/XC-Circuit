const FORMATS = {
  datetime: "Date and time",
  date: "Date only",
  time: "Time (12:00)",
  custom: "Your own pattern (DD.MM.YYYY HH:mm)",
  iso: "ISO (2026-11-01T12:00:00Z)",
  unix: "Unix seconds",
  relative: "Discord: relative (in 5 minutes)",
  discord_t: "Discord: short time (16:20)",
  discord_T: "Discord: long time (16:20:30)",
  discord_d: "Discord: short date (20/04/2026)",
  discord_D: "Discord: long date (20 April 2026)",
  discord_f: "Discord: date and time",
  discord_F: "Discord: day, date and time",
};

const UNIT_MS = { ms: 1, s: 1000, sec: 1000, second: 1000, seconds: 1000, m: 60000, min: 60000, mins: 60000, minute: 60000, minutes: 60000, h: 3600000, hr: 3600000, hrs: 3600000, hour: 3600000, hours: 3600000, d: 86400000, day: 86400000, days: 86400000, w: 604800000, week: 604800000, weeks: 604800000 };

/** "1d 2h", "-30m", "90s", "+1 week"; a plain number counts as minutes. Empty: 0. */
function amount(text) {
  const s = String(text ?? "").trim().toLowerCase();
  if (!s) return 0;
  const sign = s.startsWith("-") ? -1 : 1;
  const body = s.replace(/^[+-]\s*/, "");
  if (/^\d+(\.\d+)?$/.test(body)) return sign * Number(body) * 60000;
  let total = 0;
  let found = false;
  for (const m of body.matchAll(/(\d+(?:\.\d+)?)\s*([a-z]+)/g)) {
    const unit = UNIT_MS[m[2]];
    if (!unit) throw new Error(`Unknown time unit "${m[2]}" in Add (use s, m, h, d or w)`);
    total += Number(m[1]) * unit;
    found = true;
  }
  if (!found) throw new Error(`"${text}" isn't an amount of time such as 1d 2h or -30m`);
  return sign * total;
}

/** The parts of a date as seen in a time zone (empty: the bot's own). */
function partsOf(d, timeZone, language) {
  const opts = { year: "numeric", month: "numeric", day: "numeric", hour: "numeric", minute: "numeric", second: "numeric", hourCycle: "h23", weekday: "long" };
  if (timeZone) opts.timeZone = timeZone;
  const p = {};
  for (const x of new Intl.DateTimeFormat("en-US", opts).formatToParts(d)) p[x.type] = x.value;
  const names = (o) => new Intl.DateTimeFormat(language || "en", { ...o, ...(timeZone ? { timeZone } : {}) }).format(d);
  return {
    Y: Number(p.year), M: Number(p.month), D: Number(p.day), H: Number(p.hour) % 24, m: Number(p.minute), s: Number(p.second),
    weekday: names({ weekday: "long" }), weekdayShort: names({ weekday: "short" }), month: names({ month: "long" }), monthShort: names({ month: "short" }),
  };
}

/** DD.MM.YYYY HH:mm and friends; text in [brackets] stays as it is. */
function pattern(d, fmt, timeZone, language) {
  const p = partsOf(d, timeZone, language);
  const two = (n) => String(n).padStart(2, "0");
  const h12 = p.H % 12 || 12;
  const tokens = {
    YYYY: String(p.Y), YY: String(p.Y).slice(-2), MMMM: p.month, MMM: p.monthShort, MM: two(p.M), M: String(p.M),
    dddd: p.weekday, ddd: p.weekdayShort, DD: two(p.D), D: String(p.D), HH: two(p.H), H: String(p.H), hh: two(h12), h: String(h12),
    mm: two(p.m), m: String(p.m), ss: two(p.s), s: String(p.s), A: p.H < 12 ? "AM" : "PM", a: p.H < 12 ? "am" : "pm",
  };
  return String(fmt || "DD.MM.YYYY HH:mm").replace(/\[([^\]]*)\]|YYYY|YY|MMMM|MMM|MM|M|dddd|ddd|DD|D|HH|H|hh|h|mm|m|ss|s|A|a/g, (t, kept) => (kept !== undefined ? kept : tokens[t]));
}

module.exports = {
  id: "core.date",
  kind: "value",
  info: {
    name: "Date & Time",
    description: "The current time (or a given date), moved by an amount such as 1d 2h, as text in any format, time zone or Discord timestamp style.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "time",
    aliases: ["Now", "Time", "Format Date", "Add Time"],
    tags: ["now", "time", "timestamp", "format", "add", "date", "time zone"],
  },
  look: {
    icon: "clock",
  },
  pure: false,
  fields: {
    date: { type: "date?", pin: true, description: "Start from this date (a date, text such as 2026-12-24 18:00, or milliseconds). Empty = now." },
    add: { type: "text", default: "", label: "Add", placeholder: "1d 2h, -30m", description: "Move it by this much: 1d 2h, 30m, -1w (s, m, h, d, w). A plain number counts as minutes." },
    addMinutes: { type: "number", default: 0, label: "Add minutes", advanced: true, description: "Older projects: minutes to add (negative goes back). Add does the same and more." },
    format: { type: "select", pin: false, default: "datetime", choices: FORMATS, description: "How the text output looks. Discord timestamps show in each reader's own time zone and language." },
    pattern: { type: "text", default: "DD.MM.YYYY HH:mm", template: false, show: { format: ["custom"] }, label: "Pattern", description: "YYYY year, MM month, DD day, HH hour, mm minute, ss second; MMMM month name, dddd weekday; hh with A for 12-hour times. [Text in brackets] stays as it is." },
    timeZone: { type: "text", default: "", template: false, label: "Time zone", placeholder: "The bot's own", show: { format: ["datetime", "date", "time", "custom"] }, description: "Such as Europe/Berlin or America/New_York. Empty: the time zone of the computer the bot runs on." },
    language: { type: "text", default: "en", template: false, advanced: true, label: "Language", show: { format: ["datetime", "date", "time", "custom"] }, description: "For names of months and weekdays and the order of the date: en, de, fr …" },
  },
  outputs: {
    date: { type: "date", description: "The date." },
    text: { type: "text", description: "The date as text, in the chosen format." },
    unix: { type: "number", description: "Unix seconds (for <t:…> timestamps and other services)." },
  },
  run({ fields: f }) {
    const start = f.date == null || f.date === "" ? Date.now() : new Date(f.date).getTime();
    if (Number.isNaN(start)) throw new Error(`"${f.date}" isn't a date`);
    const d = new Date(start + amount(f.add) + (Number(f.addMinutes) || 0) * 60000);
    const unix = Math.floor(d.getTime() / 1000);
    const tz = String(f.timeZone || "").trim() || undefined;
    const lang = String(f.language || "").trim() || undefined;
    const local = (o) => d.toLocaleString(lang, { ...o, ...(tz ? { timeZone: tz } : {}) });
    const format = f.format || "datetime";
    const text = format.startsWith("discord_")
      ? `<t:${unix}:${format.slice(8)}>`
      : {
          date: () => local({ year: "numeric", month: "2-digit", day: "2-digit" }),
          iso: () => d.toISOString(),
          time: () => local({ hour: "2-digit", minute: "2-digit" }),
          datetime: () => local({ year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }),
          custom: () => pattern(d, f.pattern, tz, lang),
          relative: () => `<t:${unix}:R>`,
          unix: () => String(unix),
        }[format]?.() ?? d.toISOString();
    return { date: d, text, unix };
  },
};
