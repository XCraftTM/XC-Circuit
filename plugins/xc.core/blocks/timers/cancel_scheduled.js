const BY = { id: "By id", key: "By key", name: "Every timer with a name" };

module.exports = {
  id: "core.cancel_scheduled",
  kind: "action",
  info: {
    name: "Cancel Scheduled",
    description: "Removes timers from Schedule before they run: one by its id or key, or all with a name.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "time",
    aliases: ["Stop timer", "Cancel reminder", "Delete timer"],
    tags: ["timer", "schedule", "cancel", "reminder", "stop"],
  },
  look: {
    icon: "clock",
  },
  fields: {
    by: { type: "select", pin: false, default: "key", choices: BY, description: "Which timers." },
    id: { type: "text", pin: true, show: { by: ["id"] }, description: "The id Schedule gave." },
    key: { type: "text", default: "", show: { by: ["key"] }, description: "The key it was scheduled with, such as remind-${user.id}." },
    name: { type: "text", default: "reminder", show: { by: ["name"] }, description: "Every timer with this name." },
  },
  outputs: {
    count: { type: "number", description: "How many were removed: 0 when there was none." },
  },
  async run({ fields, ctx }) {
    const which = fields.by === "id" ? { id: fields.id } : fields.by === "name" ? { name: fields.name } : { key: fields.key };
    const value = Object.values(which)[0];
    if (value == null || String(value).trim() === "") throw new Error(`Cancel Scheduled needs ${Object.keys(which)[0] === "id" ? "an id" : `a ${Object.keys(which)[0]}`}`);
    return { count: await ctx.timers.cancel(which) };
  },
};
