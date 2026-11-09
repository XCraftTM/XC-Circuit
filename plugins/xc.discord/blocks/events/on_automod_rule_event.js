const KINDS = { created: "Created", updated: "Changed", deleted: "Deleted" };

module.exports = {
  id: "discord.on_automod_rule_event",
  kind: "event",
  info: {
    name: "AutoMod Rule Changed",
    description: "Runs when an AutoMod rule is created, changed or deleted. (AutoMod Action runs when a rule catches something.)",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["AutoMod rule created", "AutoMod rule deleted", "Filter changed"],
    tags: ["automod", "rule", "filter", "log"],
  },
  look: {
    icon: "shield",
  },
  requires: {
    intents: ["Guilds", "AutoModerationConfiguration"],
  },
  fields: {
    kind: { type: "select", pin: false, default: "created", label: "When", choices: KINDS, description: "Which event runs it." },
  },
  outputs: {
    server: { type: "discord.guild", description: "The server." },
    name: { type: "text", description: "The rule's name." },
    enabled: { type: "boolean", description: "Whether the rule is on." },
    rule: { type: "object", description: "The rule (as it is now, or was before deleting)." },
    before: { type: "object", description: "For changes: the rule before." },
  },
  listen(ctx, emit) {
    const c = ctx.discord.client;
    const out = (rule, before = null) => emit({ name: rule.name ?? "", enabled: Boolean(rule.enabled), rule, before, server: rule.guild });
    const kind = ctx.fields.kind;
    if (kind === "updated") c.on("autoModerationRuleUpdate", (old, now) => out(now, old));
    else c.on(kind === "deleted" ? "autoModerationRuleDelete" : "autoModerationRuleCreate", (rule) => out(rule));
  },
};
