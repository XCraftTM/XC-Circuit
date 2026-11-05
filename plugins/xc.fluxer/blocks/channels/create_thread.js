const FROM = { message: "From a message", channel: "In a channel", forum: "A forum post" };
const ARCHIVE = { 60: "1 hour", 1440: "1 day", 4320: "3 days", 10080: "1 week" };

module.exports = {
  id: "fluxer.create_thread",
  kind: "action",
  info: {
    name: "Create Thread",
    description: "Starts a thread on a message or in a channel, or a post in a forum. Send Message › Send to channel then posts in it.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["Start thread", "Forum post", "New thread"],
  },
  look: {
    icon: "branch",
  },
  requires: {
    permissions: ["SendMessages"],
  },
  fields: {
    from: { type: "select", pin: false, default: "message", choices: FROM, description: "Where the thread starts." },
    message: { type: "fluxer.message", pin: "only", required: true, show: { from: ["message"] }, description: "The message to start it on." },
    channel: { type: "fluxer.channel", pin: "only", required: true, show: { from: ["channel", "forum"] }, description: "The channel, or the forum." },
    name: { type: "text", default: "Discussion", description: "The thread's name." },
    text: { type: "textarea", default: "", show: { from: ["forum"] }, label: "First message", description: "What the forum post says." },
    archive: { type: "select", pin: false, default: "1440", choices: ARCHIVE, label: "Hide after", advanced: true, description: "After how long without messages it is archived." },
  },
  outputs: {
    thread: { type: "fluxer.thread", description: "The new thread." },
  },
  async run({ fields: f, ctx }) {
    const name = String(f.name ?? "").trim().slice(0, 100) || "Thread";
    const autoArchiveDuration = Number(f.archive) || 1440;
    if (f.from === "message") {
      if (!f.message) throw new Error("Connect the message");
      return { thread: await f.message.startThread({ name, autoArchiveDuration }) };
    }
    const channel = await ctx.fluxer.channel(f.channel);
    if (!channel.threads) throw new Error("This channel can't have threads");
    if (f.from === "forum") {
      const text = String(f.text ?? "").trim();
      if (!text) throw new Error("A forum post needs a first message");
      return { thread: await channel.threads.createPost({ name, message: { content: text.slice(0, 2000) }, autoArchiveDuration }) };
    }
    return { thread: await channel.threads.create({ name, autoArchiveDuration }) };
  },
};
