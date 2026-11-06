/** A path inside the bot's files folder (data/files); nothing outside it. */
function place(ctx, p) {
  const path = require("path");
  const root = path.join(ctx.runtime.dataDir, "files");
  const full = path.resolve(root, String(p ?? "").trim().replace(/^[/\\]+/, ""));
  if (full !== root && !full.startsWith(root + path.sep)) throw new Error(`"${p}" is outside the bot's files folder: use a path such as notes/today.txt`);
  return full;
}

/** `*.png`, `report-*` as a test for names. */
function matcher(pattern) {
  const p = String(pattern ?? "").trim();
  if (!p) return () => true;
  const re = new RegExp(`^${p.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, ".")}$`, "i");
  return (name) => re.test(name);
}

module.exports = {
  id: "core.list_files",
  kind: "action",
  info: {
    name: "List Files",
    description: "The files (and folders) in a folder of the bot's files folder, newest first.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "files",
    aliases: ["Files in folder", "Directory", "Folder contents"],
    tags: ["file", "folder", "list", "directory"],
  },
  look: {
    icon: "folder",
  },
  fields: {
    folder: { type: "text", default: "", description: "A folder in the bot's files folder. Empty: the files folder itself." },
    match: { type: "text", default: "", description: "Only names like this: *.png, report-*. Empty: all." },
    deep: { type: "boolean", pin: false, default: false, advanced: true, label: "Folders inside too", description: "Also list what's in the folders inside it." },
  },
  outputs: {
    files: { type: "list<object>", description: "Each with name, path (inside the files folder), size, modified (a date) and folder (true for folders)." },
    count: { type: "number", description: "How many." },
  },
  async run({ fields, ctx }) {
    const fs = require("fs").promises;
    const path = require("path");
    const root = place(ctx, "");
    const start = place(ctx, fields.folder);
    const ok = matcher(fields.match);
    const out = [];
    const walk = async (dir) => {
      let entries;
      try {
        entries = await fs.readdir(dir, { withFileTypes: true });
      } catch (e) {
        if (e.code === "ENOENT") return;
        throw e;
      }
      for (const e of entries) {
        const full = path.join(dir, e.name);
        const stat = await fs.stat(full);
        if (ok(e.name)) out.push({ name: e.name, path: path.relative(root, full).split(path.sep).join("/"), size: e.isDirectory() ? 0 : stat.size, modified: stat.mtime, folder: e.isDirectory() });
        if (e.isDirectory() && fields.deep) await walk(full);
      }
    };
    await walk(start);
    out.sort((a, b) => b.modified - a.modified);
    return { files: out, count: out.length };
  },
};
