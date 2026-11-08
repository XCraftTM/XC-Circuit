// How much time lies between two dates: for "joined 3 days ago", account
// age checks, countdowns and how long something took.

function when(v, label) {
  if (v == null || v === "") return new Date();
  if (v instanceof Date) return v;
  if (typeof v === "object" && (v.createdAt || v.joinedAt)) return new Date(v.createdAt ?? v.joinedAt);
  const d = new Date(typeof v === "string" && /^\d{9,13}$/.test(v.trim()) ? (v.trim().length > 11 ? Number(v) : Number(v) * 1000) : v);
  if (Number.isNaN(d.getTime())) throw new Error(`${label}: "${v}" isn't a date`);
  return d;
}

/** 93784000 → "1 d 2 h 3 min" (the two or three largest parts). */
function words(ms) {
  let s = Math.round(Math.abs(ms) / 1000);
  const out = [];
  for (const [n, u] of [[86400, "d"], [3600, "h"], [60, "min"], [1, "s"]]) {
    const v = Math.floor(s / n);
    s -= v * n;
    if (v) out.push(`${v} ${u}`);
  }
  return out.length ? out.slice(0, 3).join(" ") : "0 s";
}

module.exports = {
  id: "core.time_between",
  kind: "value",
  info: {
    name: "Time Between",
    description: "How much time lies between two dates (empty = now): in days, hours, minutes and seconds, and as text such as 2 d 3 h. For “joined 3 days ago”, account age and countdowns.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "time",
    aliases: ["Date Difference", "How long ago", "Countdown", "Age"],
    tags: ["date", "difference", "ago", "since", "until", "days", "account age"],
  },
  look: {
    icon: "clock",
  },
  pure: false,
  fields: {
    from: { type: "any", pin: true, default: "", label: "From", description: "The earlier date (a date, text or milliseconds; a user or member counts from when the account was made). Empty = now." },
    to: { type: "any", pin: true, default: "", label: "To", description: "The later date. Empty = now." },
  },
  outputs: {
    ms: { type: "number", description: "Milliseconds from From to To (negative when To is earlier)." },
    seconds: { type: "number", description: "Whole seconds." },
    minutes: { type: "number", description: "Whole minutes." },
    hours: { type: "number", description: "Whole hours." },
    days: { type: "number", description: "Whole days." },
    text: { type: "text", description: "As text, such as 2 d 3 h 5 min." },
    past: { type: "boolean", label: "To is earlier", description: "Whether To lies before From." },
  },
  run({ fields: f }) {
    const ms = when(f.to, "To").getTime() - when(f.from, "From").getTime();
    const whole = (n) => Math.trunc(ms / n);
    return { ms, seconds: whole(1000), minutes: whole(60000), hours: whole(3600000), days: whole(86400000), text: words(ms), past: ms < 0 };
  },
};
