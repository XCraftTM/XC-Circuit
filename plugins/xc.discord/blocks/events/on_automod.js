module.exports = {
  id: "discord.on_automod",
  kind: "event",
  info: {
    name: "AutoMod Action",
    description: "Runs when Discord's AutoMod blocks a message, alerts or times someone out.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events",
    tags: ["filter", "bad words", "spam", "moderation"],
  },
  look: {
    icon: "shield",
  },
  requires: {
    intents: ["Guilds", "AutoModerationExecution"],
  },
  fields: {},
  outputs: {
    userId: { type: "text", label: "User id", description: "Who triggered it." },
    content: { type: "text", description: "What they wrote." },
    keyword: { type: "text", description: "The word or pattern that matched." },
    action: { type: "text", description: "block, alert or timeout." },
    ruleId: { type: "text", label: "Rule id", description: "The AutoMod rule." },
    channel: { type: "discord.channel?", description: "Where." },
    server: { type: "discord.guild", description: "The server." },
  },
  listen(ctx, emit) {
    const ACTIONS = { 1: "block", 2: "alert", 3: "timeout", 4: "block" };
    ctx.discord.client.on("autoModerationActionExecution", (x) =>
      emit({ userId: x.userId, content: x.content ?? "", keyword: x.matchedKeyword ?? x.matchedContent ?? "", action: ACTIONS[x.action.type] ?? String(x.action.type), ruleId: x.ruleId, channel: x.channel ?? null, server: x.guild }),
    );
  },
};
