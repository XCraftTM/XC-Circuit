module.exports = {
  id: "discord.create_forum_post",
  kind: "action",
  info: {
    name: "Create Forum Post",
    description: "Starts a post in a forum channel, with tags.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    tags: ["forum", "post", "thread", "suggestion"],
  },
  look: {
    icon: "message",
  },
  fields: {
    forum: { type: "discord.channel", pin: "only", required: true, description: "The forum channel." },
    title: { type: "text", required: true, default: "New post", description: "The post's title." },
    text: { type: "textarea", default: "", description: "The first message." },
    embeds: { type: "list<discord.embed>", pin: "only", multi: true, description: "Embeds for the first message." },
    tags: { type: "text", default: "", description: "Tag names, comma separated (they must exist in the forum)." },
  },
  outputs: {
    thread: { type: "discord.channel", description: "The post (a thread)." },
  },
  async run({ fields: f }) {
    const wanted = String(f.tags || "").split(",").map((s) => s.trim().toLowerCase()).filter(Boolean);
    const appliedTags = (f.forum.availableTags || []).filter((t) => wanted.includes(t.name.toLowerCase())).map((t) => t.id);
    const embeds = (f.embeds || []).flat().filter(Boolean);
    const thread = await f.forum.threads.create({ name: String(f.title).slice(0, 100), message: { content: f.text || undefined, embeds: embeds.length ? embeds : undefined }, appliedTags });
    return { thread };
  },
};
