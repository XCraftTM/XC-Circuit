module.exports = {
  id: "discord.get_reactions",
  name: "Get Reactions",
  description: "Who reacted to a message with an emoji, and how many. For giveaways and starboards.",
  author: "XC Team",
  version: "1.0.0",
  category: "messages",
  icon: "message",
  kind: "action",
  tags: ["giveaway", "starboard", "votes", "count"],
  fields: {
    message: { type: "discord.message", pin: "only", required: true, description: "The message." },
    emoji: { type: "text", required: true, default: "🎉", description: "The emoji." },
    bots: { type: "boolean", default: false, label: "Count bots", description: "Include bots (such as the bot's own reaction)." },
  },
  outputs: {
    users: { type: "list<discord.user>", description: "Everyone who reacted with it." },
    count: { type: "number", description: "How many." },
  },
  async run({ fields: f }) {
    const key = String(f.emoji).replace(/^<a?:\w+:(\d+)>$/, "$1");
    const message = f.message.partial ? await f.message.fetch() : f.message;
    const reaction = message.reactions.cache.find((r) => r.emoji.name === key || r.emoji.id === key);
    if (!reaction) return { users: [], count: 0 };
    const users = [];
    let after;
    for (;;) {
      const page = await reaction.users.fetch({ limit: 100, after });
      users.push(...page.values());
      if (page.size < 100) break;
      after = page.last().id;
    }
    const counted = users.filter((u) => f.bots || !u.bot);
    return { users: counted, count: counted.length };
  },
};
