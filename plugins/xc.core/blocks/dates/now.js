const FORMATS = {
  date: "Date only",
  iso: "ISO (2026-11-01T12:00:00Z)",
  time: "Time (12:00)",
  datetime: "Date and time",
  relative: "Discord relative (in 5 minutes)",
  unix: "Unix seconds",
};

module.exports = {
  id: "core.date",
  kind: "value",
  info: {
    name: "Date & Time",
    description: "The current time (or a given date), moved by an amount and formatted.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "time",
    aliases: ["Now", "Time"],
    tags: ["now", "time", "timestamp", "format"],
  },
  look: {
    icon: "clock",
  },
  pure: false,
  fields: {
    date: { type: "date?", pin: true, description: "Start from this date. Empty = now." },
    addMinutes: { type: "number", default: 0, label: "Add minutes", description: "Use negative numbers to go back." },
    format: { type: "select", pin: false, default: "datetime", choices: FORMATS, description: "How the text output looks." },
  },
  outputs: {
    date: { type: "date", description: "The date." },
    text: { type: "text", description: "The date as text." },
  },
  run({ fields }) {
    const d = new Date((fields.date ? new Date(fields.date).getTime() : Date.now()) + (fields.addMinutes || 0) * 60000);
    const unix = Math.floor(d.getTime() / 1000);
    const text = {
      date: d.toLocaleDateString(),
      iso: d.toISOString(),
      time: d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
      datetime: d.toLocaleString(),
      relative: `<t:${unix}:R>`,
      unix: String(unix),
    }[fields.format];
    return { date: d, text };
  },
};
