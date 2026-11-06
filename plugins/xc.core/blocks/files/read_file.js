const AS = { text: "Text", json: "JSON (an object or list)", csv: "Table (CSV: a list of rows)", lines: "Lines (a list of texts)", bytes: "Bytes (a picture, say)" };
const MISSING = { empty: "Give nothing", error: "Stop with an error" };

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

/** Rows of a CSV text as objects by its first line; `,`, `;` or tabs, "quoted" values. */
function parseCsv(text) {
  const first = text.split(/\r?\n/, 1)[0];
  const sep = [",", ";", "\t"].sort((a, b) => first.split(b).length - first.split(a).length)[0];
  const rows = [];
  let row = [];
  let cell = "";
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"' && text[i + 1] === '"') {
        cell += '"';
        i++;
      } else if (c === '"') quoted = false;
      else cell += c;
    } else if (c === '"') quoted = true;
    else if (c === sep) {
      row.push(cell);
      cell = "";
    } else if (c === "\n" || c === "\r") {
      if (c === "\r" && text[i + 1] === "\n") i++;
      row.push(cell);
      rows.push(row);
      row = [];
      cell = "";
    } else cell += c;
  }
  if (cell !== "" || row.length) {
    row.push(cell);
    rows.push(row);
  }
  const [head, ...body] = rows.filter((r) => r.some((c) => c !== ""));
  if (!head) return [];
  return body.map((r) => Object.fromEntries(head.map((h, i) => [h.trim(), r[i] ?? ""])));
}

module.exports = {
  id: "core.read_file",
  kind: "action",
  info: {
    name: "Read File",
    description: "Reads a file from the bot's files folder: as text, JSON, a table (CSV), lines or bytes.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "files",
    aliases: ["Load file", "Open file", "Read CSV", "Read JSON", "Spreadsheet"],
    tags: ["file", "read", "load", "csv", "json", "text", "spreadsheet", "table"],
  },
  look: {
    icon: "folder",
  },
  fields: {
    path: { type: "text", default: "notes.txt", required: true, description: "Where in the bot's files folder, such as notes.txt or tickets/123.json." },
    as: { type: "select", pin: false, default: "text", choices: AS, description: "What the file holds." },
    ifMissing: { type: "select", pin: false, default: "empty", choices: MISSING, advanced: true, label: "If it's missing", description: "When there's no such file: give nothing (Exists is false), or stop with an error." },
  },
  outputs: {
    content: { type: "any", description: "What's in it: a text, an object or list, rows, lines or bytes." },
    exists: { type: "boolean", description: "Whether the file is there." },
    size: { type: "number", description: "Its size in bytes." },
    path: { type: "text", description: "The whole path on disk (for Send Message's Files)." },
  },
  async run({ fields, ctx }) {
    const fs = require("fs").promises;
    const full = place(ctx, fields.path);
    let bytes;
    try {
      bytes = await fs.readFile(full);
    } catch (e) {
      if (e.code === "ENOENT" && fields.ifMissing !== "error") return { content: null, exists: false, size: 0, path: full };
      if (e.code === "ENOENT") throw new Error(`There's no file ${fields.path} in the bot's files folder`);
      if (e.code === "EISDIR") throw new Error(`${fields.path} is a folder: use List Files`);
      throw e;
    }
    const text = () => bytes.toString("utf8").replace(/^﻿/, "");
    let content;
    switch (fields.as) {
      case "json":
        try {
          content = JSON.parse(text());
        } catch (e) {
          throw new Error(`${fields.path} isn't valid JSON: ${e.message}`);
        }
        break;
      case "csv":
        content = parseCsv(text());
        break;
      case "lines":
        content = text().split(/\r?\n/).filter((l, i, all) => l !== "" || i < all.length - 1);
        break;
      case "bytes":
        content = bytes;
        break;
      default:
        content = text();
    }
    return { content, exists: true, size: bytes.length, path: full };
  },
};
