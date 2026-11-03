module.exports = {
  id: "discord.remove_reaction",
  kind: "action",
  info: {
    name: "Remove Reaction",
    description: "Removes reactions from a message: one person's, or all of one emoji, or every reaction.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "reactions",
    aliases: ["Unreact"],
    tags: ["unreact", "clear reactions"],
  },
  look: {
    icon: "message",
  },
  fields: {
    message: { type: "discord.message", pin: "only", required: true, description: "The message." },
    emoji: { type: "text", default: "", description: "The emoji. Empty removes every reaction." },
    user: { type: "discord.user?", pin: "only", description: "Only this person's reaction (with an emoji). Empty = everyone's." },
  },
  async run({ fields: f }) {
    if (!f.emoji) return void (await f.message.reactions.removeAll());
    const key = String(f.emoji).replace(/^<a?:\w+:(\d+)>$/, "$1");
    const reaction = f.message.reactions.cache.find((r) => r.emoji.name === key || r.emoji.id === key || r.emoji.toString() === f.emoji);
    if (!reaction) return;
    if (f.user) await reaction.users.remove(f.user.id);
    else await reaction.remove();
  },
};
