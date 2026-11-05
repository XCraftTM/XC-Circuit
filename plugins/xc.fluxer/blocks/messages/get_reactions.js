module.exports = {
  id: "fluxer.get_reactions",
  kind: "action",
  info: {
    name: "Who Reacted",
    description: "Gets the people who reacted to a message with an emoji: for giveaways and votes.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "reactions",
    aliases: ["Reaction users", "Giveaway", "Count reactions"],
  },
  look: {
    icon: "check",
  },
  requires: {
    permissions: ["ReadMessageHistory"],
  },
  fields: {
    message: { type: "fluxer.message", pin: "only", required: true, description: "The message." },
    emoji: { type: "text", default: "🎉", description: "The emoji: 🎉, or a custom one (<:name:id> or name:id)." },
    ignoreBots: { type: "boolean", default: true, label: "Ignore bots", description: "Leave bots out, including this one." },
    limit: { type: "number", default: 100, min: 1, max: 100, step: 1, advanced: true, description: "At most this many (up to 100)." },
  },
  outputs: {
    users: { type: "list<fluxer.user>", description: "Who reacted." },
    count: { type: "number", description: "How many." },
    random: { type: "fluxer.user?", description: "One of them by chance (a giveaway winner)." },
  },
  async run({ fields: f }) {
    if (!f.message) throw new Error("Connect the message");
    const raw = String(f.emoji ?? "").trim();
    if (!raw) throw new Error("Give the emoji");
    const custom = /^<a?:(\w+):(\d+)>$/.exec(raw);
    const emoji = custom ? `${custom[1]}:${custom[2]}` : raw;
    let users = await f.message.fetchReactionUsers(emoji, { limit: Math.min(100, Math.max(1, Math.floor(f.limit || 100))) });
    if (f.ignoreBots) users = users.filter((u) => !u.bot);
    return { users, count: users.length, random: users.length ? users[Math.floor(Math.random() * users.length)] : null };
  },
};
