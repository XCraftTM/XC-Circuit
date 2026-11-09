// Discord's channel kinds (discord.js ChannelType), as people call them.
const KINDS = {
  0: "Text",
  1: "Direct message",
  2: "Voice",
  3: "Group direct message",
  4: "Category",
  5: "Announcement",
  10: "Announcement thread",
  11: "Thread",
  12: "Private thread",
  13: "Stage",
  14: "Directory",
  15: "Forum",
  16: "Media",
};
const THREADS = [10, 11, 12];
const VOICE = [2, 13];

/** A manager's or collection's items as a list (a channel's members can be either). */
function items(x) {
  if (!x) return [];
  if (x.cache instanceof Map) return [...x.cache.values()];
  if (x instanceof Map) return [...x.values()];
  return Array.isArray(x) ? x : [];
}

/** Who is in the channel: in a voice channel, who's connected; in a thread, who joined; else who can see it. */
function members(c) {
  const list = items(c.members);
  // A thread's members are thread members: the server member, where known.
  return list.map((m) => (m && m.guildMember !== undefined ? m.guildMember : m)).filter(Boolean);
}

/** The channel's pinned messages, newest first (the first 50). */
async function pinned(c) {
  if (c.messages?.fetchPins) return (await c.messages.fetchPins()).items.map((p) => p.message);
  return c.messages?.fetchPinned ? items(await c.messages.fetchPinned()) : [];
}

const INFO = {
  name: ["Name", "text", (c) => c.name ?? ""],
  id: ["Id", "text", (c) => c.id],
  mention: ["Mention", "text", (c) => `<#${c.id}>`],
  url: ["Link", "text", (c) => c.url ?? ""],
  kind: ["Kind", "text", (c) => KINDS[c.type] ?? "Other"],
  topic: ["Topic", "text", (c) => c.topic ?? ""],
  createdAt: ["Created at", "date", (c) => c.createdAt ?? null],
  server: ["Server", "discord.guild?", (c) => c.guild ?? null],
  category: ["Category", "discord.channel?", (c) => (THREADS.includes(c.type) ? null : c.parent ?? null)],
  parent: ["Channel of the thread", "discord.channel?", (c) => (THREADS.includes(c.type) ? c.parent ?? null : null)],
  position: ["Position", "number", (c) => c.position ?? 0],
  nsfw: ["Age-restricted", "boolean", (c) => Boolean(c.nsfw)],
  slowmode: ["Slowmode (seconds)", "number", (c) => c.rateLimitPerUser ?? 0],
  isThread: ["Is a thread", "boolean", (c) => THREADS.includes(c.type)],
  isVoice: ["Is a voice channel", "boolean", (c) => VOICE.includes(c.type)],
  isCategory: ["Is a category", "boolean", (c) => c.type === 4],
  members: ["Members", "list<discord.member>", members],
  memberCount: ["Member count", "number", (c) => (THREADS.includes(c.type) ? c.memberCount ?? members(c).length : members(c).length)],
  channels: ["Channels in the category", "list<discord.channel>", (c) => items(c.children).sort((a, b) => (a.position ?? 0) - (b.position ?? 0))],
  threads: ["Active threads", "list<discord.channel>", async (c) => (c.threads?.fetchActive ? [...(await c.threads.fetchActive()).threads.values()] : items(c.threads))],
  lastMessage: ["Last message", "discord.message?", async (c) => (c.lastMessageId && c.messages ? await c.messages.fetch(c.lastMessageId).catch(() => null) : null)],
  pinned: ["Pinned messages", "list<discord.message>", pinned],
  bitrate: ["Bitrate", "number", (c) => c.bitrate ?? 0],
  userLimit: ["User limit (0 = none)", "number", (c) => c.userLimit ?? 0],
  owner: ["Thread starter", "discord.user?", async (c) => (THREADS.includes(c.type) && c.ownerId ? await c.client.users.fetch(c.ownerId).catch(() => null) : null)],
  archived: ["Archived", "boolean", (c) => Boolean(c.archived)],
  locked: ["Locked", "boolean", (c) => Boolean(c.locked)],
  tags: ["Forum tags", "list<text>", (c) => (c.availableTags ?? []).map((t) => t.name)],
};
const CHOICES = Object.fromEntries(Object.entries(INFO).map(([k, [label]]) => [k, label]));

module.exports = {
  id: "discord.channel_info",
  kind: "value",
  info: {
    name: "Channel Info",
    description: "Reads one piece of information about a channel or thread: its name, kind, topic, category, members, last message and more.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["Channel details", "Channel name", "Channel topic", "Thread info"],
    tags: ["channel", "thread", "topic", "category", "slowmode", "members"],
  },
  look: {
    icon: "hash",
  },
  fields: {
    channel: { type: "discord.channel", pin: "only", required: true, description: "The channel or thread to read from." },
    info: { type: "select", pin: false, default: "name", choices: CHOICES, description: "What to read." },
  },
  outputs: {
    result: { type: "text", description: "The value you picked." },
  },
  define({ values }) {
    const [label, type] = INFO[values.info] || INFO.name;
    return { outputs: { result: { type, label, description: `The channel's ${label.toLowerCase()}.` } } };
  },
  async run({ fields }) {
    if (!fields.channel) throw new Error("Connect the channel");
    const [, , read] = INFO[fields.info] || INFO.name;
    return { result: await read(fields.channel) };
  },
};
