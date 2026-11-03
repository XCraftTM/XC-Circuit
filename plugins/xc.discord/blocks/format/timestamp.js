const STYLES = { R: "Relative (in 5 minutes)", t: "Short time (16:20)", T: "Long time (16:20:30)", d: "Short date (20/04/2026)", D: "Long date (20 April 2026)", f: "Date and time", F: "Day, date and time" };

module.exports = {
  id: "discord.timestamp",
  kind: "value",
  info: {
    name: "Timestamp",
    description: "A time that each reader sees in their own time zone and language, such as “in 5 minutes”.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "format",
    tags: ["time", "date", "countdown", "relative"],
  },
  look: {
    icon: "clock",
  },
  fields: {
    date: { type: "any", pin: true, description: "A date, a number of milliseconds since 1970, or text. Empty = now." },
    style: { type: "select", pin: false, default: "R", choices: STYLES, description: "How it is shown." },
  },
  outputs: {
    text: { type: "text", description: "The timestamp, such as <t:1767225600:R>." },
  },
  run({ fields: f }) {
    const d = f.date == null || f.date === "" ? new Date() : new Date(f.date);
    if (Number.isNaN(d.getTime())) throw new Error(`"${f.date}" isn't a date`);
    return { text: `<t:${Math.floor(d.getTime() / 1000)}:${f.style || "R"}>` };
  },
};
