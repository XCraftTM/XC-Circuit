const KEEP = { keep: "Keep as is", on: "On", off: "Off" };
const tri = (v) => (v === "on" ? true : v === "off" ? false : undefined);

module.exports = {
  id: "discord.edit_channel",
  name: "Edit Channel",
  description: "Changes a channel's name, topic, slowmode and more. Empty fields stay as they are.",
  author: "XC Team",
  version: "1.0.0",
  category: "channels",
  icon: "hash",
  kind: "action",
  tags: ["rename", "topic", "slowmode", "move"],
  fields: {
    channel: { type: "discord.channel", pin: "only", required: true, description: "The channel to change." },
    name: { type: "text", default: "", description: "New name. Empty keeps it." },
    topic: { type: "text", default: "", description: "New topic. Empty keeps it." },
    slowmode: { type: "number", default: -1, min: -1, max: 21600, step: 1, label: "Slowmode (s)", description: "Seconds between messages per person. 0 turns it off, -1 keeps it." },
    nsfw: { type: "select", pin: false, default: "keep", choices: KEEP, label: "Age-restricted", description: "Marks it as NSFW." },
    category: { type: "discord.channel?", pin: "only", description: "Move it into this category. Empty keeps it." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  outputs: {
    channel: { type: "discord.channel", description: "The changed channel." },
  },
  async run({ fields: f }) {
    const edit = { reason: f.reason || undefined };
    if (f.name) edit.name = String(f.name).slice(0, 100);
    if (f.topic) edit.topic = String(f.topic).slice(0, 1024);
    if (f.slowmode >= 0) edit.rateLimitPerUser = Math.floor(f.slowmode);
    if (tri(f.nsfw) !== undefined) edit.nsfw = tri(f.nsfw);
    if (f.category) edit.parent = f.category.id ?? f.category;
    return { channel: await f.channel.edit(edit) };
  },
};
