module.exports = {
  id: "core.on_start",
  kind: "event",
  info: {
    name: "Bot Started",
    description: "Runs once when the bot has started and is connected.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_bot",
    aliases: ["On Start", "Bot ready", "Bot gestartet", "Beim Start"],
  },
  look: {
    icon: "bolt",
  },
  outputs: {
    startedAt: { type: "date", label: "Started at", description: "When the bot started." },
  },
  listen(ctx, emit) {
    ctx.runtime.onStart(() => emit({ startedAt: new Date() }));
  },
};
