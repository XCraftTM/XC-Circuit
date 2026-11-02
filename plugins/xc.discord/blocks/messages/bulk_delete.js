module.exports = {
  id: "discord.bulk_delete",
  name: "Bulk Delete",
  description: "Deletes many recent messages at once (purge). Discord only allows messages under 14 days old.",
  author: "XC Team",
  version: "1.0.0",
  category: "messages",
  icon: "trash",
  kind: "action",
  tags: ["purge", "clear", "clean", "prune"],
  fields: {
    channel: { type: "discord.channel", pin: "only", required: true, description: "The channel to clean." },
    count: { type: "number", default: 10, min: 1, max: 100, step: 1, description: "How many of the latest messages to look at (up to 100)." },
    user: { type: "discord.user?", pin: "only", label: "Only from", description: "Only this person's messages. Empty = everyone's." },
    contains: { type: "text", default: "", advanced: true, description: "Only messages containing this text." },
    bots: { type: "boolean", default: false, advanced: true, label: "Only bots", description: "Only messages from bots." },
  },
  outputs: {
    deleted: { type: "number", description: "How many were deleted." },
  },
  async run({ fields: f }) {
    const recent = await f.channel.messages.fetch({ limit: Math.max(1, Math.min(100, Math.floor(f.count || 10))) });
    const userId = f.user?.id;
    const text = String(f.contains || "").toLowerCase();
    const pick = recent.filter((m) => (!userId || m.author.id === userId) && (!f.bots || m.author.bot) && (!text || m.content.toLowerCase().includes(text)));
    const gone = await f.channel.bulkDelete(pick, true);
    return { deleted: gone.size };
  },
};
