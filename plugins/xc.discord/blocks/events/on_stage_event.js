const KINDS = { created: "A stage starts", updated: "A stage's topic changes", deleted: "A stage ends" };

module.exports = {
  id: "discord.on_stage_event",
  kind: "event",
  info: {
    name: "Stage Started / Ended",
    description: "Runs when a stage goes live, its topic changes, or it ends.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "voice",
    aliases: ["Stage live", "Stage instance", "Stage ended"],
    tags: ["stage", "voice", "event", "live"],
  },
  look: {
    icon: "bolt",
  },
  requires: {
    intents: ["Guilds"],
  },
  fields: {
    kind: { type: "select", pin: false, default: "created", label: "When", choices: KINDS, description: "Which event runs it." },
  },
  outputs: {
    topic: { type: "text", description: "The stage's topic." },
    beforeTopic: { type: "text", label: "Topic before", description: "For changes: the topic before." },
    channel: { type: "discord.channel?", description: "The stage channel." },
    stage: { type: "object", description: "The stage instance." },
    server: { type: "discord.guild?", description: "The server." },
  },
  listen(ctx, emit) {
    const c = ctx.discord.client;
    const out = (stage, before = stage) => emit({ topic: stage.topic ?? "", beforeTopic: before?.topic ?? "", channel: stage.channel ?? null, stage, server: stage.guild ?? null });
    const kind = ctx.fields.kind;
    if (kind === "updated") c.on("stageInstanceUpdate", (old, now) => out(now, old));
    else c.on(kind === "deleted" ? "stageInstanceDelete" : "stageInstanceCreate", (stage) => out(stage));
  },
};
