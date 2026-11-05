module.exports = {
  id: "discord.get_messages",
  kind: "action",
  info: {
    name: "Get Messages",
    description: "Reads a channel's latest messages, such as for a ticket transcript.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "messages",
    aliases: ["Message history", "Fetch messages"],
    tags: ["history", "fetch", "transcript", "read"],
  },
  look: {
    icon: "message",
  },
  requires: {
    permissions: ["ReadMessageHistory"],
  },
  fields: {
    channel: { type: "discord.channel", pin: "only", required: true, description: "The channel." },
    count: { type: "number", default: 50, min: 1, max: 1000, step: 1, description: "How many (up to 1000, read 100 at a time)." },
    before: { type: "text", default: "", advanced: true, label: "Before message id", description: "Only messages older than this one." },
    user: { type: "discord.user?", pin: "only", label: "Only from", description: "Only this person's messages." },
  },
  outputs: {
    messages: { type: "list<discord.message>", description: "The messages, oldest first." },
    transcript: { type: "text", description: "One line per message: [time] name: text." },
    count: { type: "number", description: "How many." },
  },
  async run({ fields: f }) {
    const want = Math.max(1, Math.min(1000, Math.floor(f.count || 50)));
    const all = [];
    let before = f.before || undefined;
    while (all.length < want) {
      const page = await f.channel.messages.fetch({ limit: Math.min(100, want - all.length), before });
      if (!page.size) break;
      all.push(...page.values());
      before = page.last().id;
      if (page.size < 100) break;
    }
    const messages = all.filter((m) => !f.user || m.author.id === f.user.id).reverse();
    const transcript = messages
      .map((m) => {
        const extra = [m.embeds.length ? `${m.embeds.length} embed(s)` : "", m.attachments.size ? [...m.attachments.values()].map((a) => a.url).join(" ") : ""].filter(Boolean).join(" ");
        return `[${m.createdAt.toISOString().replace("T", " ").slice(0, 16)}] ${m.author.tag}: ${m.content}${extra ? " " + extra : ""}`;
      })
      .join("\n");
    return { messages, transcript, count: messages.length };
  },
};
