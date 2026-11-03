const MODES = { interval: "Every so often", daily: "Every day at", weekly: "Every week on" };
const UNITS = { seconds: "Seconds", minutes: "Minutes", hours: "Hours" };
const DAYS = { "1": "Monday", "2": "Tuesday", "3": "Wednesday", "4": "Thursday", "5": "Friday", "6": "Saturday", "0": "Sunday" };
const MS = { seconds: 1000, minutes: 60000, hours: 3600000 };

/** The next time after `now` that matches HH:MM (and the weekday, if given), on this computer's clock. */
function nextAt(now, time, weekday) {
  const [h, m] = String(time || "09:00").split(":").map((n) => Number(n) || 0);
  const at = new Date(now);
  at.setHours(h, m, 0, 0);
  if (weekday != null) at.setDate(at.getDate() + ((Number(weekday) - at.getDay() + 7) % 7));
  if (at <= now) at.setDate(at.getDate() + (weekday != null ? 7 : 1));
  return at;
}

module.exports = {
  id: "core.every",
  kind: "event",
  info: {
    name: "Every",
    description: "Runs again and again: every so many minutes, every day at a time, or every week. Times use the clock of the computer the bot runs on.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_bot",
    aliases: ["Timer", "Interval", "Schedule", "Zeitplan", "Wiederkehrend"],
    tags: ["timer", "schedule", "interval", "cron", "daily", "repeat"],
  },
  look: {
    icon: "clock",
  },
  fields: {
    mode: { type: "select", pin: false, default: "interval", choices: MODES, description: "How often." },
    every: { type: "number", pin: false, default: 10, min: 1, show: { mode: ["interval"] }, description: "How many units between runs." },
    unit: { type: "select", pin: false, default: "minutes", choices: UNITS, show: { mode: ["interval"] }, description: "Seconds, minutes or hours." },
    day: { type: "select", pin: false, default: "1", choices: DAYS, show: { mode: ["weekly"] }, description: "Which day." },
    time: { type: "text", pin: false, default: "09:00", show: { mode: ["daily", "weekly"] }, description: "The time, as HH:MM (24 hours)." },
    now: { type: "boolean", pin: false, default: false, label: "Also at start", description: "Also run once right when the bot starts." },
  },
  outputs: {
    time: { type: "date", description: "When it ran." },
    count: { type: "number", description: "How many times it has run, starting at 1." },
  },
  listen(ctx, emit) {
    const f = ctx.fields;
    let count = 0;
    const fire = () => emit({ time: new Date(), count: ++count });
    ctx.runtime.onStart(() => {
      if (f.now) fire();
      if (f.mode === "interval") {
        setInterval(fire, Math.max(1000, (Number(f.every) || 1) * (MS[f.unit] || 60000)));
        return;
      }
      const schedule = () => {
        const wait = nextAt(new Date(), f.time, f.mode === "weekly" ? f.day : null) - Date.now();
        // Long waits are split, because timers can't wait more than about 24 days.
        setTimeout(() => (wait > 2 ** 31 - 1 ? schedule() : (fire(), schedule())), Math.min(wait, 2 ** 31 - 1));
      };
      schedule();
    });
  },
};
