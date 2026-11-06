const ACTIONS = { exists: "Check that it's there", delete: "Delete", copy: "Copy", move: "Move or rename", folder: "Make a folder" };

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
  id: "core.file_action",
  kind: "action",
  info: {
    name: "File Action",
    description: "Checks that a file is there, deletes, copies or moves it, or makes a folder, in the bot's files folder.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "files",
    aliases: ["Delete file", "Copy file", "Move file", "Rename file", "File exists", "Make folder"],
    tags: ["file", "delete", "copy", "move", "rename", "exists", "folder", "mkdir"],
  },
  look: {
    icon: "folder",
  },
  fields: {
    action: { type: "select", pin: false, default: "exists", choices: ACTIONS, description: "What to do." },
    path: { type: "text", default: "", required: true, description: "The file (or folder) in the bot's files folder." },
    to: { type: "text", default: "", required: true, show: { action: ["copy", "move"] }, description: "Where to: a path in the bot's files folder. Folders are made as needed." },
  },
  outputs: {
    exists: { type: "boolean", description: "Whether it's there (after the action: false once deleted)." },
    path: { type: "text", description: "The whole path on disk of the result." },
  },
  async run({ fields, ctx }) {
    const fs = require("fs").promises;
    const path = require("path");
    const full = place(ctx, fields.path);
    if (full === place(ctx, "") && fields.action !== "exists" && fields.action !== "folder") throw new Error("Choose a file or folder inside the files folder");
    const there = (p) => fs.stat(p).then(() => true, () => false);
    switch (fields.action) {
      case "delete":
        await fs.rm(full, { recursive: true, force: true });
        return { exists: false, path: full };
      case "copy":
      case "move": {
        const to = place(ctx, fields.to);
        if (!(await there(full))) throw new Error(`There's no ${fields.path} in the bot's files folder`);
        await fs.mkdir(path.dirname(to), { recursive: true });
        if (fields.action === "copy") await fs.cp(full, to, { recursive: true });
        else await fs.rename(full, to);
        return { exists: true, path: to };
      }
      case "folder":
        await fs.mkdir(full, { recursive: true });
        return { exists: true, path: full };
      default:
        return { exists: await there(full), path: full };
    }
  },
};
