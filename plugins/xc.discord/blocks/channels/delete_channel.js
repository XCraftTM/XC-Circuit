module.exports = {
  id: "discord.delete_channel",
  name: "Delete Channel",
  description: "Deletes a channel, thread or category. The bot needs Manage Channels.",
  author: "XC Team",
  version: "1.0.0",
  category: "channels",
  icon: "trash",
  kind: "action",
  tags: ["remove channel", "close ticket"],
  fields: {
    channel: { type: "discord.channel", pin: "only", required: true, description: "The channel to delete." },
    after: { type: "number", default: 0, min: 0, label: "After (s)", description: "Wait this many seconds first (to let people read a last message)." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  async run({ fields, ctx }) {
    if (fields.after > 0) await ctx.util.sleep(fields.after * 1000);
    await fields.channel.delete(fields.reason || undefined);
  },
};
