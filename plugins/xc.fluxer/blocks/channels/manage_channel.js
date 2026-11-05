const ACTIONS = { create: "Create a channel", edit: "Change a channel", delete: "Delete a channel" };
// fluxer.js ChannelType values.
const KINDS = { 0: "Text", 2: "Voice", 4: "Category", 5: "Announcements" };

module.exports = {
  id: "fluxer.manage_channel",
  kind: "action",
  info: {
    name: "Create, Change or Delete Channel",
    description: "Creates a channel, renames it, sets its topic or slow mode, or deletes it. The bot needs Manage channels.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["Create channel", "Delete channel", "Rename channel", "Slowmode", "Channel topic"],
  },
  look: {
    icon: "hash",
  },
  requires: {
    permissions: ["ManageChannels"],
  },
  fields: {
    action: { type: "select", pin: false, default: "create", choices: ACTIONS, description: "What to do." },
    server: { type: "fluxer.server", pin: "only", required: true, show: { action: ["create"] }, description: "Where to create it." },
    channel: { type: "fluxer.channel", pin: "only", required: true, show: { action: ["edit", "delete"] }, description: "The channel." },
    name: { type: "text", default: "", show: { action: ["create", "edit"] }, description: "Its name. When changing, empty keeps it." },
    kind: { type: "select", pin: false, default: "0", choices: KINDS, label: "Type", show: { action: ["create"] }, description: "What kind of channel." },
    topic: { type: "text", default: "", show: { action: ["create", "edit"] }, description: "What it is for, shown at the top. When changing, empty keeps it." },
    parent: { type: "text", default: "", advanced: true, label: "Category id", show: { action: ["create", "edit"] }, description: "The category to put it in. Empty: none (or keep it)." },
    slowmode: {
      type: "number",
      default: -1,
      min: -1,
      max: 21600,
      advanced: true,
      label: "Slow mode (s)",
      show: { action: ["edit"] },
      description: "Seconds people wait between messages. 0 turns it off, -1 keeps it.",
    },
  },
  outputs: {
    channel: { type: "fluxer.channel?", description: "The created or changed channel." },
  },
  async run({ fields, ctx }) {
    const name = String(fields.name ?? "").trim();
    const topic = String(fields.topic ?? "").trim();
    const parentId = ctx.fluxer.id(fields.parent, "channel");
    if (fields.action === "create") {
      if (!fields.server) throw new Error("Connect the server");
      if (!name) throw new Error("Give the channel a name");
      const channel = await fields.server.createChannel({ name, type: Number(fields.kind) || 0, topic: topic || undefined, parentId: parentId || undefined });
      return { channel };
    }
    const channel = await ctx.fluxer.channel(fields.channel);
    if (fields.action === "delete") {
      await channel.delete();
      return { channel: null };
    }
    const changes = {};
    if (name) changes.name = name;
    if (topic) changes.topic = topic;
    if (parentId) changes.parentId = parentId;
    let changed = Object.keys(changes).length ? await channel.edit(changes) : channel;
    const slow = Math.round(Number(fields.slowmode));
    if (slow >= 0 && typeof channel.setSlowmode === "function") changed = await channel.setSlowmode(slow);
    return { channel: changed ?? channel };
  },
};
