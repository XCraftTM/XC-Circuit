const DIRS = { parse: "Text to time (10m → 600000 ms)", write: "Time to text (600000 ms → 10 min)" };
const UNIT_MS = { ms: 1, s: 1000, sec: 1000, second: 1000, seconds: 1000, m: 60000, min: 60000, mins: 60000, minute: 60000, minutes: 60000, h: 3600000, hr: 3600000, hrs: 3600000, hour: 3600000, hours: 3600000, d: 86400000, day: 86400000, days: 86400000, w: 604800000, week: 604800000, weeks: 604800000 };

/** "1h 30m", "90s", "2 days", "1.5h"; a plain number counts as seconds. */
function parse(text) {
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
  if (!found) throw new Error(`"${text}" isn't a duration such as 10m or 1h 30m`);
  return total;
}

function write(ms) {
  let s = Math.round(Math.abs(Number(ms) || 0) / 1000);
  const parts = [];
  for (const [n, u] of [[86400, "d"], [3600, "h"], [60, "min"], [1, "s"]]) {
    const v = Math.floor(s / n);
    s -= v * n;
    if (v) parts.push(`${v} ${u}`);
  }
  return parts.length ? parts.join(" ") : "0 s";
}

module.exports = {
  id: "core.duration",
  kind: "value",
  info: {
    name: "Duration",
    description: "Turns text such as 10m or 1h 30m into milliseconds, or milliseconds into text such as 1 h 30 min.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "dates",
    tags: ["time", "minutes", "seconds", "ms", "timeout length"],
  },
  look: {
    icon: "clock",
  },
  fields: {
    direction: { type: "select", pin: false, default: "parse", choices: DIRS, description: "Which way to convert." },
    input: { type: "any", pin: true, default: "10m", description: "Text such as 10m, 2h or 1d 12h; or a number of milliseconds." },
  },
  outputs: {
    ms: { type: "number", description: "Milliseconds." },
    seconds: { type: "number", description: "Seconds." },
    minutes: { type: "number", description: "Minutes." },
    text: { type: "text", description: "As text, such as 1 h 30 min." },
  },
  run({ fields: f }) {
    const ms = f.direction === "write" ? Number(f.input) || 0 : parse(f.input);
    return { ms, seconds: ms / 1000, minutes: ms / 60000, text: write(ms) };
  },
};
