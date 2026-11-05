// Builds types/discordjs.classes.json from discord.js's own type definitions:
// for each class a text can reach from the Discord pin types, every property,
// getter and read-only method with its type, inherited members included. The
// editor offers them after a dot in ${…}: ${#1.user.username}.
//
// Usage: node plugins/xc.discord/tools/discordjs-catalog.js [path/to/node_modules/discord.js]
// (default: the copy XC Circuit installed for running bots).
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");

// The class behind each Discord pin type.
const ROOTS = {
  "discord.user": "User",
  "discord.member": "GuildMember",
  "discord.message": "Message",
  "discord.channel": "TextChannel",
  "discord.guild": "Guild",
  "discord.role": "Role",
  "discord.interaction": "BaseInteraction",
  "discord.thread": "ThreadChannel",
  "discord.voice_channel": "VoiceChannel",
  "discord.invite": "Invite",
  "discord.poll": "Poll",
  "discord.webhook": "Webhook",
};
// How many steps from a pin type the catalog follows (user → dmChannel → …).
const DEPTH = 4;
// Members never offered: the bot's own client, internals and big caches.
const SKIP = new Set(["client", "constructor", "toJSON", "valueOf", "equals", "inspect", "freeze"]);

const pkg = path.resolve(process.argv[2] || defaultPackage());
const version = JSON.parse(fs.readFileSync(path.join(pkg, "package.json"), "utf8")).version;
const source = fs.readFileSync(path.join(pkg, "typings", "index.d.ts"), "utf8");

function defaultPackage() {
  const base = process.platform === "win32" ? process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming") : path.join(os.homedir(), ".local", "share");
  return path.join(base, "XC Circuit", "packages", "node_modules", "discord.js");
}

// ---- Reading the declarations ------------------------------------------------

/** Splits on `sep` outside brackets, parentheses, braces and angle brackets. */
function splitTop(text, sep) {
  const out = [];
  let depth = 0;
  let cur = "";
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if ("(<[{".includes(c)) depth++;
    else if (")>]}".includes(c) && text[i - 1] !== "=") depth--;
    if (depth === 0 && text.startsWith(sep, i)) {
      out.push(cur);
      cur = "";
      i += sep.length - 1;
    } else cur += c;
  }
  out.push(cur);
  return out.map((s) => s.trim()).filter(Boolean);
}

/** The bases in an `extends A<X>, B` clause, by name. */
const bases = (clause) => (clause ? splitTop(clause, ",").map((b) => b.replace(/<.*$/s, "").trim()) : []);

const classes = new Map(); // name → { bases: [], members: Map }
const aliases = new Map(); // type alias → its text
const lines = source.split("\n");
let i = 0;
while (i < lines.length) {
  const line = lines[i];
  const decl = line.match(/^export (?:declare )?(?:abstract )?(class|interface) (\w+)(.*)$/);
  const alias = line.match(/^export type (\w+)(?:<[^=]*>)? = (.*)$/);
  if (alias) {
    let text = alias[2];
    while (!text.trimEnd().endsWith(";") && i + 1 < lines.length) text += " " + lines[++i].trim();
    aliases.set(alias[1], text.replace(/;\s*$/, "").trim());
    i++;
    continue;
  }
  if (!decl) {
    i++;
    continue;
  }
  // The head may span lines up to the opening brace.
  let head = decl[3];
  while (!head.includes("{") && i + 1 < lines.length) head += " " + lines[++i].trim();
  const ext = head.match(/\bextends\s+(.*?)(?:\bimplements\b|\{)/s);
  const entry = classes.get(decl[2]) || { bases: [], members: new Map() };
  entry.bases.push(...bases(ext && ext[1]));
  classes.set(decl[2], entry);
  if (head.trimEnd().endsWith("{}")) {
    i++;
    continue;
  }
  // The body: members are indented once; long ones wrap onto deeper lines.
  i++;
  let doc = "";
  let deprecated = false;
  while (i < lines.length && !/^}/.test(lines[i])) {
    const l = lines[i];
    if (/^\s*\/\*\*/.test(l)) {
      let text = l;
      while (!text.includes("*/") && i + 1 < lines.length) text += "\n" + lines[++i];
      deprecated = /@deprecated/.test(text);
      doc = text
        .replace(/\/\*\*|\*\//g, "")
        .split("\n")
        .map((s) => s.replace(/^\s*\*\s?/, "").trim())
        .filter((s) => s && !s.startsWith("@"))
        .join(" ")
        .replace(/\{@link ([^}]+)\}/g, "$1");
      i++;
      continue;
    }
    if (/^ {2}\S/.test(l)) {
      let stmt = l.trim();
      let depth = 0;
      const count = (s) => {
        for (const c of s) depth += "({[".includes(c) ? 1 : ")}]".includes(c) ? -1 : 0;
      };
      count(stmt);
      while ((depth > 0 || !stmt.endsWith(";")) && i + 1 < lines.length && !/^}/.test(lines[i + 1])) {
        const next = lines[++i].trim();
        count(next);
        stmt += " " + next;
      }
      const m = member(stmt);
      if (m && !deprecated && !entry.members.has(m.name)) entry.members.set(m.name, { ...m, doc });
      doc = "";
      deprecated = false;
    }
    i++;
  }
  i++;
}

/** A member declaration as { name, kind, type, call }, or null for ones a text can't use. */
function member(stmt) {
  if (/^(private|protected|static|constructor|\[|#)/.test(stmt) || /^public static /.test(stmt)) return null;
  const s = stmt.replace(/^(public |readonly |abstract |declare )+/, "").replace(/;$/, "");
  const getter = s.match(/^get (\w+)\(\): (.+)$/);
  if (getter) return { name: getter[1], kind: "getter", type: getter[2] };
  const name = s.match(/^(\w+)\??/);
  if (!name) return null;
  const rest = s.slice(name[0].length);
  if (rest.startsWith(":")) return { name: name[1], kind: "property", type: rest.slice(1).trim() };
  // A method: its parameters (in balanced parentheses), then its return type.
  let j = rest.startsWith("<") ? matching(rest, 0, "<", ">") + 1 : 0;
  if (rest[j] !== "(") return null;
  const close = matching(rest, j, "(", ")");
  const params = splitTop(rest.slice(j + 1, close), ",");
  const ret = rest.slice(close + 1).replace(/^\s*:\s*/, "");
  const required = params.filter((p) => !/^\w+\?/.test(p) && !p.startsWith("...") && !/=/.test(p));
  // Read-only: no required arguments, nothing to wait for, a value back.
  const call = required.length === 0 && !/^Promise</.test(ret) && !/^(void|this|never)$/.test(ret) && !/^(fetch|delete|edit|set|create|remove|add|send|reply|clear|kick|ban|leave|pin|unpin|react|crosspost|suppress|disable|enable|toggle|move|end|stop|destroy|sync|lock|unlock)/i.test(name[1]);
  return call ? { name: name[1], kind: "method", type: ret } : null;
}

function matching(text, from, open, close) {
  let depth = 0;
  for (let k = from; k < text.length; k++) {
    if (text[k] === open) depth++;
    else if (text[k] === close && text[k - 1] !== "=" && --depth === 0) return k;
  }
  return text.length;
}

// ---- Types ---------------------------------------------------------------------

/** A member's type without null and undefined, and whether it can be empty. */
function clean(type) {
  const parts = splitTop(type, "|");
  const kept = parts.filter((p) => p !== "null" && p !== "undefined");
  return { text: kept.join(" | ") || type, optional: kept.length < parts.length };
}

const PRIMITIVE = [
  [/^(string|Snowflake|`.*`|\w*(Mention|String|URL|Hash|Url)|Locale|LocaleString)$/, "Text"],
  [/^(number|bigint)$/, "Number"],
  [/^(boolean|true|false|\w+ is .*)$/s, "Yes/no"],
  [/^Date$/, "Date"],
];

/** The class a type leads to (following aliases and the first class of a union). */
function classOf(type, seen = new Set()) {
  for (const part of splitTop(clean(type).text, "|")) {
    const name = part.replace(/^Readonly<(.*)>$/, "$1").replace(/<.*$/s, "").trim();
    if (classes.has(name)) return name;
    if (aliases.has(name) && !seen.has(name)) {
      seen.add(name);
      const found = classOf(aliases.get(name), seen);
      if (found) return found;
    }
  }
  return null;
}

/** How a type is shown: Text, Number, Yes/no, Date, a class name, or the TypeScript text. */
function shown(type) {
  const t = clean(type).text;
  for (const [re, name] of PRIMITIVE) if (re.test(t)) return name;
  return classOf(t) || (t.length > 40 ? t.slice(0, 39) + "…" : t);
}

/** Every member of a class, its bases' first, then its own (own ones win). */
function flatten(name, seen = new Set()) {
  const out = new Map();
  if (seen.has(name) || !classes.has(name)) return out;
  seen.add(name);
  const c = classes.get(name);
  for (const b of c.bases) for (const [k, v] of flatten(b, seen)) out.set(k, v);
  for (const [k, v] of c.members) out.set(k, v);
  return out;
}

// ---- The catalog -----------------------------------------------------------------

const catalog = {};
const readMethods = new Set();
const queue = Object.values(ROOTS).map((name) => [name, 0]);
while (queue.length) {
  const [name, depth] = queue.shift();
  if (catalog[name]) continue;
  const members = [];
  for (const m of [...flatten(name).values()].sort((a, b) => a.name.localeCompare(b.name))) {
    if (SKIP.has(m.name) || m.name.startsWith("_")) continue;
    const target = classOf(m.type);
    const entry = { name: m.name, kind: m.kind, type: shown(m.type) };
    if (clean(m.type).optional) entry.optional = true;
    if (target && depth + 1 < DEPTH) {
      entry.class = target;
      queue.push([target, depth + 1]);
    }
    if (m.doc) entry.doc = m.doc.length > 160 ? m.doc.slice(0, 157) + "…" : m.doc;
    if (m.kind === "method") readMethods.add(m.name);
    members.push(entry);
  }
  catalog[name] = { members };
}
// A member pointing at a class past the depth limit leads nowhere.
for (const c of Object.values(catalog)) for (const m of c.members) if (m.class && !catalog[m.class]) delete m.class;

const out = path.join(__dirname, "..", "types", "discordjs.classes.json");
const file = { source: `discord.js ${version}`, roots: ROOTS, classes: catalog };
fs.writeFileSync(out, JSON.stringify(file, null, 1) + "\n");

const count = Object.values(catalog).reduce((n, c) => n + c.members.length, 0);
console.log(`${Object.keys(catalog).length} classes, ${count} members (${readMethods.size} read-only methods) from discord.js ${version} → ${path.relative(process.cwd(), out)} (${(fs.statSync(out).size / 1024).toFixed(0)} KB)`);
