module.exports = {
  id: "core.on_start",
  name: "Bot Started",
  description: "Runs once when the bot has started and is connected.",
  author: "XC Team",
  version: "1.0.0",
  category: "events",
  icon: "bolt",
  kind: "event",
  outputs: {
    startedAt: { type: "date", label: "Started at", description: "When the bot started." },
  },
  listen(ctx, emit) {
    ctx.runtime.onStart(() => emit({ startedAt: new Date() }));
  },
};
