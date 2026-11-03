// Components V2: a whole message built from components. The design is kept as
// Discord's own JSON (made in the Layout Builder), so every component and
// setting Discord has can be used. ${…} in its texts holds JavaScript.

/** A copy of a JSON value with `fn` applied to every string. */
function mapStrings(v, fn) {
  if (typeof v === "string") return fn(v);
  if (Array.isArray(v)) return v.map((x) => mapStrings(x, fn));
  if (v && typeof v === "object") return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, mapStrings(x, fn)]));
  return v;
}

/** Calls `fn` with every { url } holder of pictures and files, nested ones included. */
function eachMedia(list, fn) {
  for (const c of list || []) {
    if (c.type === 11 && c.media) fn(c.media);
    if (c.type === 12) for (const item of c.items || []) if (item.media) fn(item.media);
    if (c.type === 13 && c.file) fn(c.file);
    if (c.accessory) eachMedia([c.accessory], fn);
    eachMedia(c.components, fn);
  }
}

module.exports = {
  id: "discord.build_layout",
  description:
    "Designs a whole message with Components V2: containers, text, sections with a picture or button, galleries, files, separators, buttons and menus. Connect it to Send Message set to Layout.",
  kind: "action",
  info: {
    name: "Build Layout",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "design",
    aliases: ["Components V2", "Container"],
    tags: ["components v2", "container", "section", "gallery", "embed"],
  },
  look: {
    icon: "layers",
    width: 300,
  },
  fields: {
    layout: {
      type: "layout",
      pin: false,
      // Names used in ${…} in the design's texts get an input, unless earlier blocks provide them.
      template: true,
      label: "Layout",
      default: {
        components: [
          {
            type: 17,
            accent_color: 0x00d8f0,
            components: [
              { type: 10, content: "## Hello ${user}!" },
              { type: 10, content: "This message is built from components." },
            ],
          },
        ],
      },
      description: "The message design. Open the Layout Builder to edit it.",
    },
  },
  outputs: {
    layout: { type: "discord.layout", description: "The finished message. Connect it to Send Message › Layout." },
  },

  run({ fields, ctx }) {
    const path = require("path");
    const components = mapStrings((fields.layout && fields.layout.components) || [], (s) => ctx.fill(s, fields));
    // Pictures and files that are not on the web (file paths, or a user's avatar
    // given as a file) are attached and pointed at with attachment://.
    const files = [];
    eachMedia(components, (m) => {
      const url = String(m.url || "").trim();
      if (!url || /^(https?|attachment):\/\//i.test(url)) return;
      let name = path.basename(url) || "file";
      while (files.some((f) => f.name === name)) name = `${files.length}-${name}`;
      files.push({ attachment: url, name });
      m.url = `attachment://${name}`;
    });
    return { layout: { components, files } };
  },
};
