// Fluxer's channel kinds, as people call them.
const KINDS = { 0: "Text", 1: "Direct message", 2: "Voice", 3: "Group direct message", 4: "Category", 998: "Link", 999: "Personal notes" };

/** The server the channel is in. */
function serverOf(c, ctx) {
  return c.guild ?? (c.guildId ? ctx.fluxer?.client?.guilds?.get?.(String(c.guildId)) : null) ?? null;
}

/** The server's channels (known ones; fetched when none are). */
async function serverChannels(c, ctx) {
  const server = serverOf(c, ctx);
  if (!server) return [];
  const known = server.channels ? [...server.channels.values()] : [];
  return known.length || typeof server.fetchChannels !== "function" ? known : await server.fetchChannels();
}

const INFO = {
  name: ["Name", "text", (c) => c.name ?? ""],
  id: ["Id", "text", (c) => String(c.id)],
  mention: ["Mention", "text", (c) => `<#${c.id}>`],
  kind: ["Kind", "text", (c) => KINDS[c.type] ?? "Other"],
  topic: ["Topic", "text", (c) => c.topic ?? ""],
  url: ["Link (link channels)", "text", (c) => (c.type === 998 ? c.url ?? "" : "")],
  createdAt: ["Created at", "date", (c) => c.createdAt ?? null],
  server: ["Server", "fluxer.server?", serverOf],
  category: ["Category", "fluxer.channel?", async (c, ctx) => (c.parentId ? (await serverChannels(c, ctx)).find((x) => String(x.id) === String(c.parentId)) ?? null : null)],
  position: ["Position", "number", (c) => c.position ?? 0],
  nsfw: ["Age-restricted", "boolean", (c) => Boolean(c.nsfw)],
  slowmode: ["Slowmode (seconds)", "number", (c) => c.rateLimitPerUser ?? 0],
  isVoice: ["Is a voice channel", "boolean", (c) => c.type === 2],
  isCategory: ["Is a category", "boolean", (c) => c.type === 4],
  channels: ["Channels in the category", "list<fluxer.channel>", async (c, ctx) => (c.type === 4 ? (await serverChannels(c, ctx)).filter((x) => String(x.parentId) === String(c.id)).sort((a, b) => (a.position ?? 0) - (b.position ?? 0)) : [])],
  lastMessage: ["Last message", "fluxer.message?", async (c) => (c.lastMessageId && c.messages?.fetch ? await c.messages.fetch(String(c.lastMessageId)).catch(() => null) : null)],
  pinned: ["Pinned messages", "list<fluxer.message>", async (c) => (typeof c.fetchPinnedMessages === "function" ? [...(await c.fetchPinnedMessages())] : [])],
  bitrate: ["Bitrate", "number", (c) => c.bitrate ?? 0],
  userLimit: ["User limit (0 = none)", "number", (c) => c.userLimit ?? 0],
};
const CHOICES = Object.fromEntries(Object.entries(INFO).map(([k, [label]]) => [k, label]));

module.exports = {
  id: "fluxer.channel_info",
  kind: "value",
  info: {
    name: "Channel Info",
    description: "Reads one piece of information about a channel: its name, kind, topic, category, last message and more.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["Channel details", "Channel name", "Channel topic"],
    tags: ["channel", "topic", "category", "slowmode"],
  },
  look: {
    icon: "hash",
  },
  fields: {
    channel: { type: "fluxer.channel", pin: "only", required: true, description: "The channel to read from." },
    info: { type: "select", pin: false, default: "name", choices: CHOICES, description: "What to read." },
  },
  outputs: {
    result: { type: "text", description: "The value you picked." },
  },
  define({ values }) {
    const [label, type] = INFO[values.info] || INFO.name;
    return { outputs: { result: { type, label, description: `The channel's ${label.toLowerCase()}.` } } };
  },
  async run({ fields, ctx }) {
    if (!fields.channel) throw new Error("Connect the channel");
    const [, , read] = INFO[fields.info] || INFO.name;
    return { result: await read(fields.channel, ctx) };
  },
};
