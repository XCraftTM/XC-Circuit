const AS = { auto: "As it is", text: "Text", json: "JSON", csv: "Table (CSV)" };
const MODES = { replace: "Replace the file", append: "Add to its end" };

/** A path inside the bot's files folder (data/files); nothing outside it. */
function place(ctx, p) {
  const path = require("path");
  const root = path.join(ctx.runtime.dataDir, "files");
  const full = path.resolve(root, String(p ?? "").trim().replace(/^[/\\]+/, ""));
  if (full !== root && !full.startsWith(root + path.sep)) throw new Error(`"${p}" is outside the bot's files folder: use a path such as notes/today.txt`);
  return full;
}

const cell = (v) => {
  const s = v == null ? "" : typeof v === "object" ? JSON.stringify(v) : String(v);
  return /[",\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
};

/** Rows (objects) as CSV, with a first line of column names unless `head` is false. */
function toCsv(rows, head = true) {
  const list = (Array.isArray(rows) ? rows : [rows]).filter((r) => r != null);
  const keys = [...new Set(list.flatMap((r) => (typeof r === "object" ? Object.keys(r) : ["value"])))];
  const lines = list.map((r) => keys.map((k) => cell(typeof r === "object" ? r[k] : r)).join(","));
  return (head ? [keys.map(cell).join(","), ...lines] : lines).join("\n") + "\n";
}

module.exports = {
  id: "core.write_file",
  kind: "action",
  info: {
    name: "Write File",
    description: "Saves text, JSON, a table (CSV) or bytes (a picture from Build Canvas) as a file in the bot's files folder, or adds to its end.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "files",
    aliases: ["Save file", "Write CSV", "Write JSON", "Append to file", "Log to file"],
    tags: ["file", "write", "save", "append", "csv", "json", "text", "log"],
  },
  look: {
    icon: "folder",
  },
  fields: {
    path: { type: "text", default: "notes.txt", required: true, description: "Where in the bot's files folder, such as notes.txt or tickets/123.json. Folders are made as needed." },
    content: { type: "any", pin: true, description: "What to save: a text, an object or list, rows for a table, or a picture." },
    as: { type: "select", pin: false, default: "auto", choices: AS, description: "As it is: texts as text, pictures as bytes, objects and lists as JSON." },
    mode: { type: "select", pin: false, default: "replace", choices: MODES, description: "Adding to a table leaves out the column names when the file has them already." },
  },
  outputs: {
    path: { type: "text", description: "The whole path on disk (for Send Message's Files)." },
    size: { type: "number", description: "The file's size now, in bytes." },
  },
  async run({ fields, ctx }) {
    const fs = require("fs").promises;
    const path = require("path");
    const full = place(ctx, fields.path);
    await fs.mkdir(path.dirname(full), { recursive: true });
    const v = fields.content;
    const append = fields.mode === "append";
    const exists = await fs.stat(full).then((s) => s.size > 0, () => false);
    let data;
    const as = fields.as === "auto" ? (Buffer.isBuffer(v) || Buffer.isBuffer(v?.attachment) ? "bytes" : v !== null && typeof v === "object" ? "json" : "text") : fields.as;
    switch (as) {
      case "bytes":
        data = Buffer.isBuffer(v) ? v : v.attachment;
        break;
      case "json":
        data = JSON.stringify(v ?? null, null, 2) + (append ? "\n" : "");
        break;
      case "csv":
        data = toCsv(v, !(append && exists));
        break;
      default: {
        const text = v == null ? "" : typeof v === "object" ? JSON.stringify(v) : String(v);
        data = text;
        if (append) {
          // Added text starts on a line of its own, and ends one.
          const last = exists ? (await fs.readFile(full)).subarray(-1).toString() : "\n";
          data = `${last === "\n" ? "" : "\n"}${text}${/\n$/.test(text) ? "" : "\n"}`;
        }
      }
    }
    if (append) await fs.appendFile(full, data);
    else await fs.writeFile(full, data);
    return { path: full, size: (await fs.stat(full)).size };
  },
};
