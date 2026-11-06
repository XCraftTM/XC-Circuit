module.exports = {
  id: "core.list_scheduled",
  kind: "action",
  info: {
    name: "List Scheduled",
    description: "The timers from Schedule that haven't run yet, soonest first: all, or those with a name.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "time",
    aliases: ["Timers", "Reminders", "Upcoming"],
    tags: ["timer", "schedule", "list", "reminder", "upcoming"],
  },
  look: {
    icon: "clock",
  },
  fields: {
    name: { type: "text", default: "", description: "Only timers with this name. Empty: all of them." },
    key: { type: "text", default: "", advanced: true, description: "Only the timer with this key." },
  },
  outputs: {
    timers: { type: "list<object>", description: "Each with id, name, key, due (a date) and data." },
    first: { type: "object?", description: "The one that runs next, or nothing." },
    count: { type: "number", description: "How many." },
  },
  async run({ fields, ctx }) {
    const key = String(fields.key || "").trim();
    const timers = (await ctx.timers.list(String(fields.name || "").trim() || undefined))
      .filter((t) => !key || t.key === key)
      .map((t) => ({ id: t.id, name: t.name, key: t.key ?? null, due: new Date(t.due), data: t.data }));
    return { timers, first: timers[0], count: timers.length };
  },
};
