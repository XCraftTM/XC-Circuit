const ACTIONS = { list: "List a forum's tags", add: "Add a tag to a forum", remove: "Remove a tag from a forum", apply: "Set a post's tags" };

/** Tags of a forum by name (or id). */
function tagIds(forum, names) {
  const wanted = String(names || "")
    .split(/[,\n]/)
    .map((s) => s.trim().toLowerCase())
    .filter(Boolean);
  return wanted.map((w) => {
    const tag = forum.availableTags.find((t) => t.name.toLowerCase() === w || t.id === w);
    if (!tag) throw new Error(`The forum has no tag ${w}`);
    return tag.id;
  });
}

module.exports = {
  id: "discord.forum_tags",
  kind: "action",
  info: {
    name: "Forum Tags",
    description: "Lists, adds and removes a forum channel's tags, or sets which tags a post has (such as Solved for support forums). The bot needs Manage Channels, or Manage Threads to tag others' posts.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["Tag post", "Mark solved", "Forum tag"],
    tags: ["forum", "tag", "post", "thread", "solved", "support"],
  },
  look: {
    icon: "tag",
  },
  fields: {
    action: { type: "select", pin: false, default: "apply", choices: ACTIONS, description: "What to do." },
    forum: { type: "discord.channel?", pin: "only", show: { action: ["list", "add", "remove"] }, description: "The forum channel." },
    post: { type: "discord.channel?", pin: "only", show: { action: ["apply"] }, description: "The forum post (thread)." },
    tags: { type: "text", default: "", show: { action: ["add", "remove", "apply"] }, description: "Tag names, separated by commas. For apply: the post's tags afterwards (up to 5); empty = none." },
    emoji: { type: "text", default: "", show: { action: ["add"] }, description: "An emoji for a new tag (optional)." },
    modOnly: { type: "boolean", pin: false, default: false, show: { action: ["add"] }, label: "Only moderators", description: "Only members who can manage threads can use it." },
  },
  outputs: {
    tags: { type: "list<object>", description: "The forum's tags (id, name, emoji, modOnly), or the post's tags after apply." },
    names: { type: "list<text>", description: "Their names." },
  },
  async run({ fields: f }) {
    const out = (tags) => ({ tags: tags.map((t) => ({ id: t.id, name: t.name, emoji: t.emoji?.name ?? "", modOnly: Boolean(t.moderated) })), names: tags.map((t) => t.name) });
    if (f.action === "apply") {
      const post = f.post;
      if (!post?.setAppliedTags || !post.parent?.availableTags) throw new Error("Connect a forum post");
      const ids = tagIds(post.parent, f.tags).slice(0, 5);
      await post.setAppliedTags(ids);
      return out(post.parent.availableTags.filter((t) => ids.includes(t.id)));
    }
    const forum = f.forum;
    if (!forum?.availableTags) throw new Error("Connect a forum channel");
    let tags = [...forum.availableTags];
    if (f.action === "add") {
      const names = String(f.tags || "").split(",").map((s) => s.trim()).filter(Boolean);
      const emoji = String(f.emoji || "").trim();
      const custom = /<a?:\w+:(\d+)>/.exec(emoji);
      for (const name of names) {
        if (tags.some((t) => t.name.toLowerCase() === name.toLowerCase())) continue;
        tags.push({ name: name.slice(0, 20), moderated: Boolean(f.modOnly), emoji: custom ? { id: custom[1], name: null } : emoji ? { id: null, name: emoji } : null });
      }
      const changed = await forum.setAvailableTags(tags.slice(0, 20));
      return out([...changed.availableTags]);
    }
    if (f.action === "remove") {
      const ids = tagIds(forum, f.tags);
      tags = tags.filter((t) => !ids.includes(t.id));
      const changed = await forum.setAvailableTags(tags);
      return out([...changed.availableTags]);
    }
    return out(tags);
  },
};
