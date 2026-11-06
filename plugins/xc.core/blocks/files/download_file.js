/** A path inside the bot's files folder (data/files); nothing outside it. */
function place(ctx, p) {
  const path = require("path");
  const root = path.join(ctx.runtime.dataDir, "files");
  const raw = String(p ?? "").trim();
  // A whole path on any system (/home/…, C:\…) is refused, not read as one inside the folder.
  if (path.posix.isAbsolute(raw) || path.win32.isAbsolute(raw)) throw new Error(`"${p}" is a path on the computer: use one inside the bot's files folder, such as notes/today.txt`);
  const full = path.resolve(root, raw);
  if (full !== root && !full.startsWith(root + path.sep)) throw new Error(`"${p}" is outside the bot's files folder: use a path such as notes/today.txt`);
  return full;
}

module.exports = {
  id: "core.download_file",
  kind: "action",
  info: {
    name: "Download File",
    description: "Saves a file from the web, or a message's attachment, into the bot's files folder.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "files",
    aliases: ["Save attachment", "Download", "Fetch file"],
    tags: ["file", "download", "attachment", "save", "url"],
  },
  look: {
    icon: "folder",
  },
  fields: {
    from: { type: "any", default: "https://", label: "From", description: "A web address, or an attachment of a message." },
    path: { type: "text", default: "downloads/", description: "Where in the bot's files folder. Ending in / (or empty): that folder, with the file's own name." },
    maxMb: { type: "number", default: 25, min: 1, max: 500, advanced: true, label: "At most (MB)", description: "Bigger files aren't saved: the block stops with an error." },
    privateOk: { type: "boolean", pin: false, default: false, advanced: true, label: "Private addresses too", description: "An address from ${…} or a wire may only go to the public web, not this computer or its network (people could otherwise make the bot reach those). On: anywhere, for addresses only you choose." },
  },
  outputs: {
    path: { type: "text", description: "The whole path on disk (for Send Message's Files)." },
    name: { type: "text", description: "The file's name." },
    size: { type: "number", description: "Its size in bytes." },
  },
  async run({ fields, ctx }) {
    const fs = require("fs").promises;
    const path = require("path");
    const f = fields.from;
    const url = String((f && typeof f === "object" ? f.url || f.proxyURL : f) ?? "").trim();
    if (!/^https?:\/\//i.test(url)) throw new Error("Download File needs a web address (https://…) or an attachment");
    const own = (f && typeof f === "object" && (f.name || f.filename)) || decodeURIComponent(new URL(url).pathname.split("/").pop() || "") || "download";
    const name = String(own).replace(/[\\/:*?"<>|]/g, "_");
    const target = String(fields.path ?? "").trim();
    const full = place(ctx, !target || /[\\/]$/.test(target) ? `${target}${name}` : target);
    const max = (Number(fields.maxMb) || 25) * 1024 * 1024;
    // An address the author typed goes anywhere; one from ${…} or a wire (an attachment, say) only to the public web.
    const publicOnly = !ctx.authored("from") && !fields.privateOk;
    const { res, body: bytes } = await ctx.util.fetch(url, { signal: AbortSignal.timeout(60000) }, { publicOnly, maxBytes: max });
    if (!res.ok) throw new Error(`${new URL(url).host} answered ${res.status}`);
    await fs.mkdir(path.dirname(full), { recursive: true });
    await fs.writeFile(full, bytes);
    return { path: full, name: path.basename(full), size: bytes.length };
  },
};
