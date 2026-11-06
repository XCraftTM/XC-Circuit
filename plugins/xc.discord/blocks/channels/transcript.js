// Create Transcript: a channel's messages as one HTML file that looks like
// Discord (embeds, Components V2, replies, threads, polls), for a ticket
// that's closed or a channel that's archived. With better-discord-transcripts,
// always rendered in full on the bot (hydrate), so the file needs no scripts
// from the web and names or messages can't put code into it.

module.exports = {
  id: "discord.create_transcript",
  kind: "action",
  info: {
    name: "Create Transcript",
    description: "Saves a channel's messages as an HTML file that looks like Discord, such as a ticket before it's closed. Send it with Send Message's Files, or keep it with Write File.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["Transcript", "Ticket log", "Save channel", "Export messages", "Archive channel", "HTML transcript"],
    tags: ["transcript", "ticket", "archive", "html", "log", "export", "messages", "modmail"],
  },
  look: {
    icon: "message",
  },
  requires: {
    // Pinned: a package checked by hand for this block (see the commit).
    dependencies: { "better-discord-transcripts": "2.0.0" },
    intents: ["GuildMessages", "MessageContent"],
    permissions: ["ViewChannel", "ReadMessageHistory"],
  },
  fields: {
    channel: { type: "discord.channel", pin: "only", required: true, description: "The channel (or thread) to save." },
    limit: { type: "number", pin: false, default: 1000, min: 0, max: 100000, label: "At most", description: "How many of the latest messages. 0: all of them (slow in long channels)." },
    name: { type: "text", default: "transcript-${channel.name}.html", label: "File name", description: "The file's name, ending in .html." },
    images: { type: "boolean", pin: false, default: false, label: "Keep pictures", description: "Put pictures into the file, so it still shows them when Discord's links expire. Makes it much bigger." },
    bots: { type: "boolean", pin: false, default: true, advanced: true, label: "Bots' messages too", description: "Off: leave out what bots wrote (the ticket's buttons, say)." },
  },
  outputs: {
    file: { type: "any", description: "The transcript, as a file. Connect it to Send Message's Files, or Write File." },
    count: { type: "number", description: "How many messages it holds." },
    size: { type: "number", description: "The file's size in bytes." },
  },
  async run({ fields, ctx }) {
    const channel = fields.channel;
    if (!channel?.messages || typeof channel.isTextBased !== "function" || !channel.isTextBased()) throw new Error("Connect a text channel or thread");
    const t = ctx.require("better-discord-transcripts");
    let counted = 0;
    const limit = Math.max(0, Number(fields.limit) || 0);
    const name = String(fields.name || "transcript.html").replace(/[\\/:*?"<>|]/g, "_").replace(/(\.html?)?$/i, ".html");
    const html = await t.createTranscript(channel, {
      limit: limit || -1,
      filter: (m) => (fields.bots === false && m.author?.bot ? false : (counted++, true)),
      returnType: t.ExportReturnType.Buffer,
      filename: name,
      saveImages: Boolean(fields.images),
      poweredBy: false,
      // Rendered in full here: no scripts from the web, and profiles not put into a script.
      hydrate: true,
    });
    return { file: { attachment: html, name }, count: limit ? Math.min(counted, limit) : counted, size: html.length };
  },
};
