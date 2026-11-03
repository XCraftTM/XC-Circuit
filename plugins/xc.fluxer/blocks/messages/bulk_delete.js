module.exports = {
  id: "fluxer.bulk_delete",
  kind: "action",
  info: {
    name: "Delete Messages",
    description: "Deletes the latest messages in a channel at once, as a purge command does.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["Purge", "Clear messages", "Bulk delete"],
  },
  look: {
    icon: "trash",
  },
  fields: {
    channel: { type: "fluxer.channel", pin: "only", required: true, description: "The channel." },
    count: { type: "number", default: 10, min: 1, max: 100, description: "How many of the latest messages (1 to 100)." },
  },
  outputs: {
    deleted: { type: "number", description: "How many were deleted." },
  },
  async run({ fields, ctx }) {
    const channel = await ctx.fluxer.channel(fields.channel);
    if (typeof channel.bulkDelete !== "function") throw new Error("Messages can only be deleted at once in server text channels");
    const count = Math.min(100, Math.max(1, Math.round(Number(fields.count) || 0)));
    const ids = await channel.bulkDelete(count);
    return { deleted: Array.isArray(ids) ? ids.length : count };
  },
};
