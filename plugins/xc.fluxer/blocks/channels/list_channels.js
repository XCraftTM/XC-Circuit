const KINDS = { all: "Every kind", 0: "Text", 2: "Voice", 4: "Categories", 5: "Announcements" };

module.exports = {
  id: "fluxer.list_channels",
  kind: "action",
  info: {
    name: "Server Channels",
    description: "Gets a server's channels, optionally only one kind, in their order.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["List channels", "All channels"],
  },
  look: {
    icon: "hash",
  },
  fields: {
    server: { type: "fluxer.server", pin: "only", required: true, description: "The server." },
    kind: { type: "select", pin: false, default: "all", choices: KINDS, description: "Which channels." },
  },
  outputs: {
    channels: { type: "list<fluxer.channel>", description: "The channels, in their order." },
    names: { type: "list<text>", description: "Their names." },
    count: { type: "number", description: "How many." },
  },
  async run({ fields: f }) {
    if (!f.server) throw new Error("Connect the server");
    let channels = [...(await f.server.fetchChannels())];
    if (f.kind !== "all") channels = channels.filter((c) => Number(c.type) === Number(f.kind));
    channels.sort((a, b) => (a.position ?? 0) - (b.position ?? 0));
    return { channels, names: channels.map((c) => c.name ?? ""), count: channels.length };
  },
};
