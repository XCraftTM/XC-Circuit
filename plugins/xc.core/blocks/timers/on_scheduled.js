module.exports = {
  id: "core.on_scheduled",
  kind: "event",
  info: {
    name: "On Scheduled",
    description: "Runs when a timer from Schedule with this name is due, once per timer.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "time",
    aliases: ["When timer ends", "Reminder due", "Timer"],
    tags: ["timer", "schedule", "reminder", "due", "later"],
  },
  look: {
    icon: "clock",
  },
  fields: {
    name: { type: "text", pin: false, required: true, default: "reminder", description: "The timers it runs for: Schedule blocks with this name." },
    catchUp: {
      type: "boolean",
      pin: false,
      default: true,
      advanced: true,
      label: "Run missed ones",
      description: "Timers that were due while the bot was off run when it starts. Off: they're dropped.",
    },
  },
  outputs: {
    data: { type: "any", description: "The value given to Schedule." },
    id: { type: "text", description: "The timer's id." },
    key: { type: "text?", description: "Its key, if it had one." },
    due: { type: "date", description: "When it was meant to run." },
    late: { type: "number", description: "How late it runs, in ms: about 0, or more when the bot was off." },
  },
  listen(ctx, emit) {
    ctx.timers.on(ctx.fields.name, (t) => emit({ data: t.data, id: t.id, key: t.key, due: t.due, late: t.late }), { catchUp: ctx.fields.catchUp !== false });
  },
};
