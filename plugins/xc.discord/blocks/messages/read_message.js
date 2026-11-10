// Read From Message: pick a part of a message step by step (its text, an
// embed, a button, a piece of its layout, an attachment, who it mentions), then what
// to take out of it (as it is, the mentions or links in it, what matches a
// pattern …). The steps that don't belong to the part picked are hidden.

const numbered = (first, more, count, extra) => ({ 1: first, ...Object.fromEntries(Array.from({ length: count - 1 }, (_, i) => [String(i + 2), `${more} ${i + 2}`])), ...extra });

const PARTS = { content: "Text", embed: "Embed", components: "Buttons and menus", layout: "Layout (Components V2)", attachment: "Attachment", mentions: "Mentions" };
const EMBED_PARTS = {
  description: ["Description", "text"],
  title: ["Title", "text"],
  text: ["All its text", "text"],
  url: ["Title link", "text"],
  color: ["Colour", "text"],
  author: ["Author name", "text"],
  authorUrl: ["Author link", "text"],
  footer: ["Footer text", "text"],
  image: ["Image URL", "text"],
  thumbnail: ["Thumbnail URL", "text"],
  timestamp: ["Timestamp", "date"],
  field: ["A field's value, by its name", "text"],
  fieldNumber: ["A field's value, by its number", "text"],
  fields: ["Its fields (name and value)", "list<object>"],
  whole: ["The whole embed", "object"],
};
const COMPONENT_PARTS = {
  label: ["Label", "text"],
  customId: ["Custom id", "text"],
  url: ["Link", "text"],
  emoji: ["Emoji", "text"],
  kind: ["Kind (button or menu)", "text"],
  disabled: ["Is disabled", "boolean"],
  options: ["A menu's options (labels)", "list<text>"],
  whole: ["The whole component", "object"],
};
// A Components V2 layout, piece by piece: what kinds of piece there are,
// and for each kind what can be read from one: [label, type, how].
const STYLES = { 1: "primary", 2: "secondary", 3: "success", 4: "danger", 5: "link", 6: "premium" };
const MENUS = { 3: "text", 5: "user", 6: "role", 7: "mentionable", 8: "channel" };
const emojiText = (e) => (e ? (e.id ? `<${e.animated ? "a" : ""}:${e.name}:${e.id}>` : e.name || "") : "");
const textsIn = (c) => walk([c]).filter((x) => x.type === 10).map((x) => x.content || "");
const LAYOUT = {
  text: {
    label: "A text",
    is: (c) => c.type === 10,
    parts: { content: ["Its text", "text", (c) => c.content || ""] },
  },
  section: {
    label: "A section",
    is: (c) => c.type === 9,
    parts: {
      text: ["All its text", "text", (c) => (c.components || []).map((x) => x.content || "").filter(Boolean).join("\n")],
      texts: ["Each of its texts", "list<text>", (c) => (c.components || []).map((x) => x.content || "")],
      accessory: ["What is beside it (button, picture or nothing)", "text", (c) => (c.accessory ? (c.accessory.type === 2 ? "button" : "picture") : "nothing")],
      accessoryLabel: ["The button's label beside it", "text", (c) => (c.accessory?.type === 2 ? c.accessory.label || "" : "")],
      accessoryId: ["The button's custom id beside it", "text", (c) => (c.accessory?.type === 2 ? c.accessory.custom_id || "" : "")],
      accessoryUrl: ["The picture's or link's URL beside it", "text", (c) => c.accessory?.media?.url || c.accessory?.url || ""],
      whole: ["The whole section", "object", (c) => c],
    },
  },
  container: {
    label: "A container",
    is: (c) => c.type === 17,
    parts: {
      text: ["All its text", "text", (c) => textsIn(c).filter(Boolean).join("\n")],
      texts: ["Each of its texts", "list<text>", (c) => textsIn(c)],
      color: ["Accent colour", "text", (c) => hex(c.accent_color)],
      spoiler: ["Is a spoiler", "boolean", (c) => Boolean(c.spoiler)],
      pieces: ["How many pieces it has", "number", (c) => (c.components || []).length],
      whole: ["The whole container", "object", (c) => c],
    },
  },
  gallery: {
    label: "A media gallery",
    is: (c) => c.type === 12,
    parts: {
      urls: ["Its pictures' URLs", "list<text>", (c) => (c.items || []).map((i) => i.media?.url).filter(Boolean)],
      descriptions: ["Its pictures' descriptions", "list<text>", (c) => (c.items || []).map((i) => i.description || "")],
      count: ["How many pictures", "number", (c) => (c.items || []).length],
      whole: ["The whole gallery", "object", (c) => c],
    },
  },
  picture: {
    label: "A picture (thumbnail or in a gallery)",
    // A thumbnail is one; a gallery is as many as it has pictures.
    each: (c) => (c.type === 11 ? [c] : c.type === 12 ? c.items || [] : []),
    parts: {
      url: ["URL", "text", (p) => p.media?.url || ""],
      description: ["Description", "text", (p) => p.description || ""],
      spoiler: ["Is a spoiler", "boolean", (p) => Boolean(p.spoiler)],
      whole: ["The whole picture", "object", (p) => p],
    },
  },
  button: {
    label: "A button",
    is: (c) => c.type === 2,
    parts: {
      label: ["Label", "text", (c) => c.label || ""],
      customId: ["Custom id", "text", (c) => c.custom_id || ""],
      url: ["Link", "text", (c) => c.url || ""],
      emoji: ["Emoji", "text", (c) => emojiText(c.emoji)],
      style: ["Style (primary, danger, link …)", "text", (c) => STYLES[c.style] || ""],
      disabled: ["Is disabled", "boolean", (c) => Boolean(c.disabled)],
      whole: ["The whole button", "object", (c) => c],
    },
  },
  menu: {
    label: "A select menu",
    is: (c) => c.type in MENUS,
    parts: {
      placeholder: ["Placeholder", "text", (c) => c.placeholder || ""],
      customId: ["Custom id", "text", (c) => c.custom_id || ""],
      options: ["Its options' labels", "list<text>", (c) => (c.options || []).map((o) => o.label)],
      values: ["Its options' values", "list<text>", (c) => (c.options || []).map((o) => o.value)],
      kind: ["What it picks (text, user, role, channel …)", "text", (c) => MENUS[c.type]],
      disabled: ["Is disabled", "boolean", (c) => Boolean(c.disabled)],
      whole: ["The whole menu", "object", (c) => c],
    },
  },
  file: {
    label: "A file",
    is: (c) => c.type === 13,
    parts: {
      url: ["URL", "text", (c) => c.file?.url || ""],
      name: ["File name", "text", (c) => c.name || String(c.file?.url || "").split(/[/?#]/).filter(Boolean).pop() || ""],
      spoiler: ["Is a spoiler", "boolean", (c) => Boolean(c.spoiler)],
      whole: ["The whole file", "object", (c) => c],
    },
  },
};
// What to read from a layout: all of it at once, or one kind of piece.
const LAYOUT_KINDS = { allText: "All its text", ...Object.fromEntries(Object.entries(LAYOUT).map(([k, v]) => [k, v.label])), whole: "The whole layout" };
const partKey = (kind) => `layout${kind[0].toUpperCase()}${kind.slice(1)}`;
const firstPart = (kind) => Object.keys(LAYOUT[kind].parts)[0];
const ATTACHMENT_PARTS = { url: ["URL", "text"], name: ["File name", "text"], size: ["Size (bytes)", "number"], type: ["File type", "text"], whole: ["The whole attachment", "object"] };
const MENTIONS = { users: ["Users", "list<discord.user>"], roles: ["Roles", "list<discord.role>"], channels: ["Channels", "list<discord.channel>"], everyone: ["@everyone or @here", "boolean"] };
// What to take out of a text: its label, and what one text becomes ("text" or a list).
const THEN = {
  raw: ["As it is", null],
  userMentions: ["User mentions in it (ids)", "list<text>"],
  roleMentions: ["Role mentions in it (ids)", "list<text>"],
  channelMentions: ["Channel mentions in it (ids)", "list<text>"],
  links: ["Links in it", "list<text>"],
  numbers: ["Numbers in it", "list<number>"],
  emoji: ["Custom emoji in it", "list<text>"],
  regex: ["What matches a pattern", null],
  lines: ["Its lines", "list<text>"],
  words: ["Its words", "list<text>"],
  trim: ["Without spaces around it", "text"],
};
const labels = (table) => Object.fromEntries(Object.entries(table).map(([k, [l]]) => [k, l]));

/** The type of what the steps picked give before the last one, and whether one or several were asked for. */
function picked(v) {
  // A step left as it starts counts as that: every row and every button, the first embed and file.
  const start = { row: "any", component: "all", embed: "1", attachment: "1" };
  const many = (key) => ["all", "any"].includes(v[key] ?? start[key]);
  const list = (type, several) => (several && !type.startsWith("list<") ? `list<${type}>` : type);
  switch (v.part) {
    case "embed":
      return list((EMBED_PARTS[v.embedPart] || EMBED_PARTS.description)[1], many("embed"));
    case "components":
      return list((COMPONENT_PARTS[v.componentPart] || COMPONENT_PARTS.label)[1], many("row") || many("component"));
    case "layout": {
      const kind = v.layoutKind in LAYOUT_KINDS ? v.layoutKind : "allText";
      if (kind === "allText") return "text";
      if (kind === "whole") return "list<object>";
      const parts = LAYOUT[kind].parts;
      return list((parts[v[partKey(kind)]] || parts[firstPart(kind)])[1], (v.layoutWhich || "1") === "all");
    }
    case "attachment":
      return list((ATTACHMENT_PARTS[v.attachmentPart] || ATTACHMENT_PARTS.url)[1], many("attachment"));
    case "mentions":
      return (MENTIONS[v.mention] || MENTIONS.users)[1];
    default:
      return "text";
  }
}

/** The result's type after the last step, which only texts have. */
function resultType(v) {
  const before = picked(v);
  if (before !== "text" && before !== "list<text>") return before;
  const then = v.then in THEN ? v.then : "raw";
  if (then === "raw") return before;
  if (then === "regex") return v.take === "all" || before === "list<text>" ? "list<text>" : "text";
  const [, one] = THEN[then];
  return one === "text" ? before : one;
}

const json = (x) => (x && typeof x.toJSON === "function" ? x.toJSON() : x);
const hex = (color) => (typeof color === "number" ? `#${color.toString(16).padStart(6, "0").toUpperCase()}` : "");
const values = (collection) => (collection ? (typeof collection.values === "function" && !Array.isArray(collection) ? [...collection.values()] : [...collection]) : []);

/** One or all of `list` by a select's value: a number from 1, "last", or "all" / "any". */
function choose(list, which) {
  if (which === "all" || which === "any") return list;
  if (which === "last") return list.slice(-1);
  const at = Number(which) - 1;
  return at >= 0 && at < list.length ? [list[at]] : [];
}

/** Every component in a tree of them (rows, sections, containers), in reading order. */
function walk(components, out = []) {
  for (const raw of components || []) {
    const c = json(raw);
    if (!c) continue;
    out.push(c);
    walk(c.components, out);
    if (c.accessory) walk([c.accessory], out);
  }
  return out;
}

function embedPart(e, v) {
  const fields = e.fields || [];
  switch (v.embedPart) {
    case "title":
      return e.title || "";
    case "text":
      return [e.author?.name, e.title, e.description, ...fields.flatMap((f) => [f.name, f.value]), e.footer?.text].filter(Boolean).join("\n");
    case "url":
      return e.url || "";
    case "color":
      return hex(e.color);
    case "author":
      return e.author?.name || "";
    case "authorUrl":
      return e.author?.url || "";
    case "footer":
      return e.footer?.text || "";
    case "image":
      return e.image?.url || "";
    case "thumbnail":
      return e.thumbnail?.url || "";
    case "timestamp":
      return e.timestamp ? new Date(e.timestamp) : null;
    case "field": {
      const name = String(v.fieldName ?? "").trim().toLowerCase();
      return fields.find((f) => String(f.name).trim().toLowerCase() === name)?.value ?? "";
    }
    case "fieldNumber":
      return fields[Number(v.fieldNumber) - 1]?.value ?? "";
    case "fields":
      return fields.map((f) => ({ name: f.name, value: f.value, inline: Boolean(f.inline) }));
    case "whole":
      return e;
    default:
      return e.description || "";
  }
}

function componentPart(c, part) {
  const menu = c.type !== 2;
  switch (part) {
    case "customId":
      return c.custom_id || "";
    case "url":
      return c.url || "";
    case "emoji":
      return emojiText(c.emoji);
    case "kind":
      return menu ? "menu" : "button";
    case "disabled":
      return Boolean(c.disabled);
    case "options":
      return (c.options || []).map((o) => o.label);
    case "whole":
      return c;
    default:
      return (menu ? c.placeholder : c.label) || "";
  }
}

/** The pieces of a layout that the steps pick, and what is read from each. */
function layoutPart(message, v) {
  const top = (message.components || []).map(json);
  const kind = v.layoutKind in LAYOUT_KINDS ? v.layoutKind : "allText";
  if (kind === "whole") return { value: top, found: top.length > 0 };
  // Everywhere in the message, or in one of its containers only.
  let scope = top;
  if (v.layoutIn && v.layoutIn !== "anywhere") {
    const container = walk(top).filter((c) => c.type === 17)[Number(v.layoutIn) - 1];
    scope = container ? container.components || [] : [];
  }
  const all = walk(scope);
  if (kind === "allText") {
    const texts = all.filter((c) => c.type === 10).map((c) => c.content || "");
    return { value: texts.join("\n"), found: texts.length > 0 };
  }
  const { is, each, parts } = LAYOUT[kind];
  const pieces = each ? all.flatMap(each) : all.filter(is);
  const chosen = choose(pieces, v.layoutWhich || "1");
  const read = (parts[v[partKey(kind)]] || parts[firstPart(kind)])[2];
  const got = chosen.map(read);
  const several = v.layoutWhich === "all";
  return { value: several ? got.flat() : got.length ? got[0] : null, found: chosen.length > 0 };
}

/** What the steps before the last one give, and whether there was anything. */
function read(message, v) {
  const one = (list, several) => (several ? list : list.length ? list[0] : null);
  switch (v.part) {
    case "embed": {
      const embeds = choose(values(message.embeds).map(json), v.embed || "1");
      const got = embeds.map((e) => embedPart(e, v));
      const several = v.embed === "all";
      return { value: one(several ? got.flat() : got, several), found: embeds.length > 0 };
    }
    case "components": {
      // Rows of buttons and menus, wherever they are (in a layout's container too).
      const rows = walk(message.components).filter((c) => c.type === 1);
      const inRows = choose(rows, v.row || "any").map((r) => (r.components || []).map(json));
      const several = (v.row || "any") === "any" || (v.component || "all") === "all";
      const chosen = inRows.flatMap((row) => choose(row, v.component || "all"));
      const got = chosen.map((c) => componentPart(c, v.componentPart));
      return { value: one(several ? got.flat() : got, several), found: chosen.length > 0 };
    }
    case "layout":
      return layoutPart(message, v);
    case "attachment": {
      const files = choose(values(message.attachments), v.attachment || "1");
      const part = (a) => ({ url: a.url, name: a.name, size: a.size, type: a.contentType || "" })[v.attachmentPart || "url"] ?? { url: a.url, name: a.name, size: a.size, type: a.contentType || "" };
      const several = v.attachment === "all";
      return { value: one(files.map(part), several), found: files.length > 0 };
    }
    case "mentions": {
      const m = message.mentions || {};
      if (v.mention === "everyone") return { value: Boolean(m.everyone), found: Boolean(m.everyone) };
      const list = values(m[v.mention in MENTIONS ? v.mention : "users"]);
      return { value: list, found: list.length > 0 };
    }
    default:
      return { value: String(message.content ?? ""), found: Boolean(message.content) };
  }
}

/** The last step on one text. */
function take(text, v) {
  const all = (pattern, group = 0) => [...text.matchAll(pattern)].map((m) => m[group]);
  switch (v.then) {
    case "userMentions":
      return all(/<@!?(\d+)>/g, 1);
    case "roleMentions":
      return all(/<@&(\d+)>/g, 1);
    case "channelMentions":
      return all(/<#(\d+)>/g, 1);
    case "links":
      return all(/https?:\/\/[^\s<>)]+/g);
    case "numbers":
      return all(/-?\d+(?:\.\d+)?/g).map(Number);
    case "emoji":
      return all(/<a?:\w+:\d+>/g);
    case "lines":
      return text.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
    case "words":
      return text.split(/\s+/).filter(Boolean);
    case "trim":
      return text.trim();
    case "regex": {
      let pattern;
      try {
        pattern = new RegExp(String(v.pattern ?? ""), v.ignoreCase === false ? "g" : "gi");
      } catch (e) {
        throw new Error(`The pattern isn't a valid regular expression: ${e.message}`);
      }
      if (!v.pattern) return v.take === "all" ? [] : "";
      const found = [...text.matchAll(pattern)];
      if (v.take === "all") return found.map((m) => m[0]);
      if (v.take === "group") return found[0]?.[1] ?? "";
      return found[0]?.[0] ?? "";
    }
    default:
      return text;
  }
}

module.exports = {
  id: "discord.read_message",
  kind: "value",
  info: {
    name: "Read From Message",
    description: "Takes one thing out of a message, picked step by step: its text, a part of an embed, a button or menu, its layout, an attachment or who it mentions. A last step can pull the mentions, links or numbers out of a text, or what matches a pattern.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "messages",
    aliases: ["Get From Message", "Message Part", "Embed Field", "Parse Message", "Extract"],
    tags: ["embed", "components", "layout", "regex", "mention", "button", "attachment"],
  },
  look: {
    icon: "message",
    width: 260,
  },
  fields: {
    message: { type: "discord.message", pin: "only", required: true, description: "The message to read from." },
    part: { type: "select", pin: false, default: "content", label: "From its", choices: PARTS, description: "Which part of the message to read." },
    embed: { type: "select", pin: false, default: "1", label: "Which embed", choices: numbered("First embed", "Embed", 10, { last: "Last embed", all: "Every embed" }), description: "A message has up to 10 embeds." },
    embedPart: { type: "select", pin: false, default: "description", label: "Its", choices: labels(EMBED_PARTS), description: "What to read from the embed." },
    fieldName: { type: "text", pin: true, default: "", label: "Field name", placeholder: "Reason", description: "The field's name, as the embed shows it (capitals don't matter)." },
    fieldNumber: { type: "number", pin: true, default: 1, min: 1, max: 25, label: "Field number", description: "1 is the embed's first field. An embed has up to 25." },
    row: { type: "select", pin: false, default: "any", label: "Which row", choices: { any: "Every row", ...numbered("Row 1", "Row", 5) }, description: "A message has up to 5 rows of buttons and menus." },
    component: { type: "select", pin: false, default: "all", label: "Which one", choices: { all: "Every button and menu", ...numbered("The first", "Number", 5) }, description: "A row has up to 5 buttons, or one menu." },
    componentPart: { type: "select", pin: false, default: "label", label: "Its", choices: labels(COMPONENT_PARTS), description: "What to read from the button or menu (a menu's label is its placeholder)." },
    layoutIn: { type: "select", pin: false, default: "anywhere", label: "Look in", choices: { anywhere: "The whole message", ...numbered("Container 1", "Container", 5) }, description: "A layout's pieces can sit in containers (the boxes with a coloured edge): look in all of the message, or in one container only." },
    layoutKind: { type: "select", pin: false, default: "allText", label: "For", choices: LAYOUT_KINDS, description: "What kind of piece to read: all the text at once, or a text, section, container, gallery, picture, button, menu or file." },
    layoutWhich: { type: "select", pin: false, default: "1", label: "Which one", choices: numbered("The first", "Number", 10, { last: "The last", all: "Every one" }), description: "Counted from the top of the message (or of the container), in reading order." },
    ...Object.fromEntries(Object.entries(LAYOUT).filter(([, k]) => Object.keys(k.parts).length > 1).map(([kind, k]) => [partKey(kind), { type: "select", pin: false, default: firstPart(kind), label: "Its", choices: labels(k.parts), description: `What to read from ${k.label.toLowerCase()}.` }])),
    attachment: { type: "select", pin: false, default: "1", label: "Which file", choices: numbered("First attachment", "Attachment", 10, { last: "Last attachment", all: "Every attachment" }), description: "A message has up to 10 attachments." },
    attachmentPart: { type: "select", pin: false, default: "url", label: "Its", choices: labels(ATTACHMENT_PARTS), description: "What to read from the file." },
    mention: { type: "select", pin: false, default: "users", label: "Who", choices: labels(MENTIONS), description: "Which mentions to give." },
    then: { type: "select", pin: false, default: "raw", label: "Take", choices: labels(THEN), description: "What to take out of the text: all of it, or only what you look for in it." },
    pattern: { type: "text", pin: true, default: "", label: "Pattern", placeholder: "#(\\d+)", description: "A regular expression. Round brackets make a group, which The first match's first group gives." },
    take: { type: "select", pin: false, default: "first", label: "Give", choices: { first: "The first match", all: "Every match", group: "The first match's first group" }, description: "Which of what matches to give." },
    ignoreCase: { type: "boolean", pin: false, default: true, label: "Ignore capitals", description: "On: A matches a." },
  },
  outputs: {
    result: { type: "text", description: "What was read." },
    found: { type: "boolean", description: "Whether the message has that part (an embed, a button, an attachment, text …)." },
  },
  define({ values }) {
    const v = { part: "content", then: "raw", ...values };
    const own = {
      embed: ["embed", "embedPart"],
      components: ["row", "component", "componentPart"],
      layout: ["layoutKind"],
      attachment: ["attachment", "attachmentPart"],
      mentions: ["mention"],
      content: [],
    };
    const shown = new Set(own[v.part] || []);
    if (v.part === "embed" && v.embedPart === "field") shown.add("fieldName");
    if (v.part === "embed" && v.embedPart === "fieldNumber") shown.add("fieldNumber");
    if (v.part === "layout") {
      const kind = v.layoutKind in LAYOUT_KINDS ? v.layoutKind : "allText";
      if (kind !== "whole") shown.add("layoutIn");
      if (kind in LAYOUT) {
        shown.add("layoutWhich");
        if (Object.keys(LAYOUT[kind].parts).length > 1) shown.add(partKey(kind));
      }
    }
    // The last step is for texts only.
    const before = picked(v);
    if (before === "text" || before === "list<text>") {
      shown.add("then");
      if (v.then === "regex") ["pattern", "take", "ignoreCase"].forEach((k) => shown.add(k));
    }
    const steps = ["embed", "embedPart", "fieldName", "fieldNumber", "row", "component", "componentPart", "layoutIn", "layoutKind", "layoutWhich", ...Object.keys(LAYOUT).map(partKey), "attachment", "attachmentPart", "mention", "then", "pattern", "take", "ignoreCase"];
    return { hide: steps.filter((k) => !shown.has(k)), outputs: { result: { type: resultType(v), description: "What was read." } } };
  },
  run({ fields }) {
    const v = { part: "content", then: "raw", ...fields };
    if (!fields.message) throw new Error("Connect a message to read from.");
    const { value, found } = read(fields.message, v);
    const before = picked(v);
    if ((before !== "text" && before !== "list<text>") || v.then === "raw" || !(v.then in THEN)) {
      return { result: value ?? (before.startsWith("list<") ? [] : before === "text" ? "" : null), found };
    }
    if (before === "list<text>") {
      const each = (value || []).map((t) => take(String(t ?? ""), v));
      // A step that gives a list for each text gives one list of them all;
      // a pattern's match for each text leaves out the texts without one.
      if (each.some(Array.isArray)) return { result: each.flat(), found };
      return { result: v.then === "regex" ? each.filter((t) => t !== "") : each, found };
    }
    return { result: take(String(value ?? ""), v), found };
  },
};
