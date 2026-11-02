// Every text in the embed may contain {placeholders}.
const TEXTS = (e) => [
  e.title,
  e.description,
  e.url,
  e.author?.name,
  e.author?.icon,
  e.author?.url,
  e.footer?.text,
  e.footer?.icon,
  e.image,
  e.thumbnail,
  e.time,
  ...(e.fields || []).flatMap((f) => [f.name, f.value]),
];

const EVENT_NAMES = ["user", "member", "channel", "server", "interaction", "message", "content", "author"];

module.exports = {
  id: "discord.build_embed",
  name: "Build Embed",
  description: "Creates a rich embed. Design it in the Embed Builder; every {placeholder} you type becomes an input.",
  author: "XC Team",
  version: "2.0.0",
  category: "messages",
  icon: "embed",
  kind: "action",
  preview: "embed",
  fields: {
    embed: {
      type: "embed",
      pin: false,
      label: "Embed",
      default: { color: "#00D8F0", title: "", description: "", fields: [] },
      description: "The embed design. Open the Embed Builder to edit it.",
    },
  },
  outputs: {
    embed: { type: "discord.embed", description: "The finished embed. Connect it to Send Message." },
  },

  define({ values }) {
    // {xp} in any text becomes an input pin called xp. Names the event already
    // provides (such as {user}) are filled automatically and get no pin.
    const add = {};
    const embed = values.embed || {};
    for (const text of TEXTS(embed)) {
      for (const [, name] of String(text || "").matchAll(/\{([A-Za-z_$][\w$]*)/g)) {
        if (EVENT_NAMES.includes(name) || add[name]) continue;
        add[name] = { type: "any", pin: true, label: `{${name}}`, description: `Fills {${name}} in the embed.` };
      }
    }
    return { add };
  },

  migrate(values, from) {
    // 1.x had one field per part of the embed.
    if (from === "1") {
      const embed = {
        color: values.color || "#00D8F0",
        title: values.title || "",
        description: values.description || "",
        footer: values.footer ? { text: values.footer } : undefined,
        image: values.image || "",
        thumbnail: values.thumbnail || "",
        timestamp: Boolean(values.timestamp),
        fields: [],
      };
      return { embed };
    }
    return values;
  },

  run({ fields, ctx }) {
    const { EmbedBuilder } = require("discord.js");
    const e = fields.embed || {};
    const fill = (s) => ctx.fill(s, fields);
    const embed = new EmbedBuilder();
    if (e.color) embed.setColor(e.color);
    if (e.title) embed.setTitle(fill(e.title).slice(0, 256));
    if (e.url) embed.setURL(fill(e.url));
    if (e.description) embed.setDescription(fill(e.description).slice(0, 4096));
    if (e.author?.name) embed.setAuthor({ name: fill(e.author.name), iconURL: e.author.icon ? fill(e.author.icon) : undefined, url: e.author.url ? fill(e.author.url) : undefined });
    for (const f of (e.fields || []).slice(0, 25)) {
      if (f.name || f.value) embed.addFields({ name: fill(f.name) || "​", value: fill(f.value) || "​", inline: Boolean(f.inline) });
    }
    if (e.footer?.text) embed.setFooter({ text: fill(e.footer.text), iconURL: e.footer.icon ? fill(e.footer.icon) : undefined });
    if (e.image) embed.setImage(fill(e.image));
    if (e.thumbnail) embed.setThumbnail(fill(e.thumbnail));
    if (e.time) {
      // A set time: a date, or a placeholder holding a date or a timestamp in ms.
      const value = fill(e.time).trim();
      const date = new Date(/^\d+$/.test(value) ? Number(value) : value);
      if (!Number.isNaN(date.getTime())) embed.setTimestamp(date);
    } else if (e.timestamp) embed.setTimestamp();
    return { embed };
  },
};
