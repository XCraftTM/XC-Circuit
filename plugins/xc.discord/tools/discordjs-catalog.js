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

/** The type arguments in `<A, B<C>>` at the start of `text` (empty without). */
function typeArgs(text) {
  const t = text.trimStart();
  if (!t.startsWith("<")) return [];
  return splitTop(t.slice(1, matching(t, 0, "<", ">")), ",");
}

/** The bases in an `extends A<X>, B` clause: their names and type arguments. */
const bases = (clause) =>
  clause
    ? splitTop(clause, ",").map((b) => {
        const name = b.replace(/<.*$/s, "").trim();
        return { name, args: typeArgs(b.slice(name.length)) };
      })
    : [];

const classes = new Map(); // name → { bases: [{ name, args }], params: [], members: Map }
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
  const entry = classes.get(decl[2]) || { bases: [], params: [], members: new Map() };
  // Its type parameters (`<Key, Holds = Thing>`: Key, Holds), for its bases' members.
  if (!entry.params.length) entry.params = typeArgs(head).map((p) => p.split(/[\s=]/)[0]);
  // The bases (only the part before the class's own `extends`: type
  // parameters can have `extends` of their own).
  const own = head.trimStart().startsWith("<") ? head.slice(head.indexOf(">", matching(head.trimStart(), 0, "<", ">")) + 1) : head;
  const ownExt = own.match(/\bextends\s+(.*?)(?:\bimplements\b|\{)/s) || ext;
  entry.bases.push(...bases(ownExt && ownExt[1]));
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

/** `text` with type parameters put in: `Collection<Key, Holds>` → `Collection<Snowflake, GuildMember>`. */
const fill = (text, subst) => (subst.size ? text.replace(/\b[A-Za-z_]\w*\b/g, (w) => (subst.has(w) ? subst.get(w) : w)) : text);

/**
 * Every member of a class, its bases' first, then its own (own ones win).
 * `subst` gives its type parameters: a manager's `cache` is then the
 * Collection of what that manager holds (GuildMemberManager: GuildMember).
 */
function flatten(name, subst = new Map(), seen = new Set()) {
  const out = new Map();
  if (seen.has(name) || !classes.has(name)) return out;
  seen.add(name);
  const c = classes.get(name);
  for (const b of c.bases) {
    const base = classes.get(b.name);
    const sub = new Map();
    if (base) base.params.forEach((p, k) => b.args[k] !== undefined && sub.set(p, fill(b.args[k], subst)));
    for (const [k, v] of flatten(b.name, sub, seen)) out.set(k, v);
  }
  for (const [k, v] of c.members) out.set(k, { ...v, type: fill(v.type, subst) });
  return out;
}

/** A Collection or Map type: which, and the type of what it holds (its values). */
function collectionOf(type) {
  const t = clean(type).text.replace(/^Readonly<(.*)>$/s, "$1");
  const m = t.match(/^(?:Readonly)?(Collection|Map)<(.*)>$/s);
  if (!m) return null;
  const args = splitTop(m[2], ",");
  return { kind: m[1], item: unwrap(args[args.length - 1]) };
}

/** discord.js's helper types around a class: CacheTypeReducer<State, Class, …> and If<Test, Class, Other>. */
function unwrap(type) {
  const t = type.trim();
  const m = t.match(/^(CacheTypeReducer|If)<(.*)>$/s);
  if (!m) return t;
  const args = splitTop(m[2], ",");
  // The class, or the first part that is one.
  const pick = args.slice(1).map(unwrap).find((a) => classOf(a)) || args[1] || t;
  return unwrap(pick);
}

// What a Collection (discord.js's Map with more) and a Map have, for any item:
// `$T` is the item's class or type, filled in by the editor (Collection<GuildMember>).
const ITEM = { type: "$T", class: "$T" };
const TEMPLATES = {
  Collection: [
    { name: "at", kind: "method", ...ITEM, doc: "The one at this place: at(0) is the first, at(-1) the last." },
    { name: "every", kind: "method", type: "Yes/no", doc: "Whether a test is true for all: every(m => !m.user.bot)." },
    { name: "filter", kind: "method", type: "Collection<$T>", class: "Collection<$T>", doc: "Only those a test picks: filter(m => !m.user.bot)." },
    { name: "find", kind: "method", ...ITEM, doc: "The first one a test picks: find(m => m.displayName === \"Ada\")." },
    { name: "first", kind: "method", ...ITEM, doc: "The first one (first(3): the first three, as a list)." },
    { name: "firstKey", kind: "method", type: "Text", doc: "The first one's key (its id)." },
    { name: "get", kind: "method", ...ITEM, doc: "The one with this key (an id): get(\"123456789012345678\")." },
    { name: "has", kind: "method", type: "Yes/no", doc: "Whether it holds one with this key (an id): has(\"123…\")." },
    { name: "last", kind: "method", ...ITEM, doc: "The last one." },
    { name: "lastKey", kind: "method", type: "Text", doc: "The last one's key (its id)." },
    { name: "map", kind: "method", type: "List", doc: "Something from each, as a list: map(m => m.displayName)." },
    { name: "random", kind: "method", ...ITEM, doc: "One picked at random." },
    { name: "size", kind: "property", type: "Number", doc: "How many it holds." },
    { name: "some", kind: "method", type: "Yes/no", doc: "Whether a test is true for any: some(m => m.user.bot)." },
    { name: "sort", kind: "method", type: "Collection<$T>", class: "Collection<$T>", doc: "Sorted by a comparison: sort((a, b) => a.joinedTimestamp - b.joinedTimestamp)." },
  ],
  Map: [
    { name: "get", kind: "method", ...ITEM, doc: "The one with this key: get(\"key\")." },
    { name: "has", kind: "method", type: "Yes/no", doc: "Whether it holds one with this key." },
    { name: "keys", kind: "method", type: "List", doc: "Its keys, one after another ([...x.keys()] for a list)." },
    { name: "size", kind: "property", type: "Number", doc: "How many it holds." },
    { name: "values", kind: "method", type: "List", doc: "What it holds, one after another ([...x.values()] for a list)." },
  ],
};

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
    const coll = collectionOf(m.type);
    const target = coll ? null : classOf(m.type);
    const entry = { name: m.name, kind: m.kind, type: shown(m.type) };
    if (clean(m.type).optional) entry.optional = true;
    if (coll) {
      // A Collection (or Map) of something: its own members, then the item's.
      const item = classOf(coll.item);
      const itemShown = item || shown(coll.item);
      entry.type = `${coll.kind}<${itemShown}>`;
      entry.class = `${coll.kind}<${itemShown}>`;
      if (item && depth + 1 < DEPTH) queue.push([item, depth + 1]);
    } else if (target && depth + 1 < DEPTH) {
      entry.class = target;
      queue.push([target, depth + 1]);
    }
    if (m.doc) entry.doc = m.doc.length > 160 ? m.doc.slice(0, 157) + "…" : m.doc;
    if (m.kind === "method") readMethods.add(m.name);
    members.push(entry);
  }
  catalog[name] = { members };
}
// A member pointing at a class past the depth limit leads nowhere (a
// Collection of one still has its own members: size, first() …).
for (const c of Object.values(catalog)) for (const m of c.members) if (m.class && !catalog[m.class] && !/^(Collection|Map)</.test(m.class)) delete m.class;
for (const [name, members] of Object.entries(TEMPLATES)) catalog[name] = { members };

const out = path.join(__dirname, "..", "types", "discordjs.classes.json");
const file = { source: `discord.js ${version}`, roots: ROOTS, classes: catalog };
fs.writeFileSync(out, JSON.stringify(file, null, 1) + "\n");

const count = Object.values(catalog).reduce((n, c) => n + c.members.length, 0);
console.log(`${Object.keys(catalog).length} classes, ${count} members (${readMethods.size} read-only methods) from discord.js ${version} → ${path.relative(process.cwd(), out)} (${(fs.statSync(out).size / 1024).toFixed(0)} KB)`);
