const FROM = { message: "From a message", channel: "In a channel" };
const ARCHIVE = { "60": "1 hour", "1440": "1 day", "4320": "3 days", "10080": "1 week" };

module.exports = {
  id: "discord.create_thread",
  name: "Create Thread",
  description: "Starts a thread from a message, or a new one in a channel (private threads for tickets).",
  author: "XC Team",
  version: "1.0.0",
  category: "channels",
  icon: "message",
  kind: "action",
  tags: ["ticket", "discussion", "reply chain"],
  fields: {
    from: { type: "select", pin: false, default: "message", choices: FROM, description: "Where the thread starts." },
    message: { type: "discord.message", pin: "only", required: true, show: { from: ["message"] }, description: "The message to start it from." },
    channel: { type: "discord.channel", pin: "only", required: true, show: { from: ["channel"] }, description: "The channel to create it in." },
    name: { type: "text", required: true, default: "Discussion", description: "The thread's name." },
    private: { type: "boolean", default: false, show: { from: ["channel"] }, description: "Only people added (or mentioned) can see it." },
    autoArchive: { type: "select", pin: false, default: "1440", choices: ARCHIVE, label: "Hide after", description: "Archive it after this long without messages." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  outputs: {
    thread: { type: "discord.channel", description: "The new thread. Send Message › Send to channel posts in it." },
  },
  async run({ fields: f }) {
    const { ChannelType } = require("discord.js");
    const options = { name: String(f.name || "Thread").slice(0, 100), autoArchiveDuration: Number(f.autoArchive) || 1440, reason: f.reason || undefined };
    if (f.from === "message") return { thread: await f.message.startThread(options) };
    options.type = f.private ? ChannelType.PrivateThread : ChannelType.PublicThread;
    return { thread: await f.channel.threads.create(options) };
  },
};
