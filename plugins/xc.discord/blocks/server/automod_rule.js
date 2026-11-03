const ACTIONS = { create: "Create a word filter", list: "List rules", delete: "Delete a rule" };

module.exports = {
  id: "discord.automod_rule",
  kind: "action",
  info: {
    name: "AutoMod Rule",
    description: "Sets up Discord's built-in AutoMod: block messages with certain words, alert a channel, or time people out. The bot needs Manage Server.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["Word filter", "Spam filter"],
    tags: ["filter", "bad words", "blocklist", "moderation"],
  },
  look: {
    icon: "shield",
  },
  fields: {
    action: { type: "select", pin: false, default: "create", choices: ACTIONS, description: "What to do." },
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    name: { type: "text", default: "Word filter", show: { action: ["create", "delete"] }, description: "The rule's name." },
    words: { type: "textarea", default: "", show: { action: ["create"] }, description: "Words to catch, one per line. *word* matches inside other words." },
    patterns: { type: "textarea", default: "", advanced: true, show: { action: ["create"] }, description: "Regular expressions, one per line." },
    allow: { type: "textarea", default: "", advanced: true, show: { action: ["create"] }, label: "Allowed", description: "Words that never count, one per line." },
    block: { type: "boolean", default: true, show: { action: ["create"] }, label: "Block the message", description: "Stop the message from being sent." },
    message: { type: "text", default: "", show: { action: ["create"] }, label: "Block message", description: "What the person is told. Empty = Discord's default." },
    alert: { type: "discord.channel?", pin: "only", show: { action: ["create"] }, label: "Alert channel", description: "Post an alert here." },
    timeout: { type: "number", default: 0, min: 0, max: 40320, show: { action: ["create"] }, label: "Time out (minutes)", description: "Also time them out. 0 = no." },
  },
  outputs: {
    rule: { type: "object", description: "The created rule." },
    rules: { type: "list<object>", description: "For List: every rule (name, id, enabled)." },
  },
  async run({ fields: f }) {
    const d = require("discord.js");
    const m = f.server.autoModerationRules;
    const lines = (t) => String(t || "").split(/\r?\n/).map((s) => s.trim()).filter(Boolean);
    if (f.action === "list") {
      const all = [...(await m.fetch()).values()];
      return { rules: all.map((r) => ({ name: r.name, id: r.id, enabled: r.enabled })), rule: null };
    }
    if (f.action === "delete") {
      const all = await m.fetch();
      const r = all.find((x) => x.name === f.name);
      if (!r) throw new Error(`No AutoMod rule named ${f.name}`);
      await r.delete();
      return { rule: null, rules: [] };
    }
    const actions = [];
    if (f.block) actions.push({ type: d.AutoModerationActionType.BlockMessage, metadata: f.message ? { customMessage: String(f.message).slice(0, 150) } : {} });
    if (f.alert) actions.push({ type: d.AutoModerationActionType.SendAlertMessage, metadata: { channel: f.alert } });
    if (f.timeout > 0) actions.push({ type: d.AutoModerationActionType.Timeout, metadata: { durationSeconds: Math.round(f.timeout * 60) } });
    if (!actions.length) throw new Error("Choose at least one: block, alert or time out");
    const rule = await m.create({
      name: String(f.name || "Word filter"),
      eventType: d.AutoModerationRuleEventType.MessageSend,
      triggerType: d.AutoModerationRuleTriggerType.Keyword,
      triggerMetadata: { keywordFilter: lines(f.words), regexPatterns: lines(f.patterns), allowList: lines(f.allow) },
      actions,
      enabled: true,
    });
    return { rule, rules: [] };
  },
};
