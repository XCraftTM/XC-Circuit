const TYPES = { string: "Options you list", user: "Users", role: "Roles", channel: "Channels", mentionable: "Users and roles" };

/** "value | Label | description | emoji" lines; a line with only a label uses it as the value too. */
function parseOptions(text) {
  return String(text || "")
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .map((line) => {
      const [a, b, description, emoji] = line.split("|").map((s) => s.trim());
      return { value: a, label: b || a, description: description || undefined, emoji: emoji || undefined };
    });
}

module.exports = {
  id: "discord.select_menu",
  kind: "value",
  info: {
    name: "Select Menu",
    description: "A dropdown to attach to a message. Picks arrive at Select Menu Used with the same custom id.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "components",
    aliases: ["Dropdown", "Auswahlmenü"],
    tags: ["dropdown", "component", "choose", "roles menu"],
  },
  look: {
    icon: "pointer",
  },
  fields: {
    menuType: { type: "select", pin: false, default: "string", label: "Choose from", choices: TYPES, description: "What people pick: options you list, or users, roles or channels of the server." },
    customId: { type: "text", default: "my-menu", label: "Custom id", description: "Identifies the menu in Select Menu Used." },
    options: { type: "textarea", default: "red | Red\nblue | Blue", show: { menuType: ["string"] }, description: "One option per line: value | Label | description | emoji. Only a label is fine too." },
    items: { type: "list<any>", pin: "only", multi: true, show: { menuType: ["string"] }, description: "More options from a list: texts, or objects with label, value and description." },
    placeholder: { type: "text", default: "Choose …", description: "Shown before anything is picked." },
    min: { type: "number", default: 1, min: 0, max: 25, step: 1, label: "Pick at least", advanced: true, description: "The fewest picks allowed (0 makes it optional)." },
    max: { type: "number", default: 1, min: 1, max: 25, step: 1, label: "Pick at most", advanced: true, description: "The most picks allowed." },
    disabled: { type: "boolean", default: false, advanced: true, description: "Shown grayed out." },
  },
  outputs: {
    menu: { type: "discord.component", description: "Connect to Send Message › Components." },
  },
  run({ fields: f }) {
    const d = require("discord.js");
    const Builder = { user: d.UserSelectMenuBuilder, role: d.RoleSelectMenuBuilder, channel: d.ChannelSelectMenuBuilder, mentionable: d.MentionableSelectMenuBuilder }[f.menuType] || d.StringSelectMenuBuilder;
    const menu = new Builder().setCustomId(String(f.customId || "menu").slice(0, 100)).setDisabled(Boolean(f.disabled));
    if (f.placeholder) menu.setPlaceholder(String(f.placeholder).slice(0, 150));
    let count = 25;
    if (Builder === d.StringSelectMenuBuilder) {
      const extra = (f.items || []).flat().filter((x) => x != null).map((x) => (typeof x === "object" ? { value: String(x.value ?? x.label), label: String(x.label ?? x.value), description: x.description, emoji: x.emoji } : { value: String(x), label: String(x) }));
      const options = [...parseOptions(f.options), ...extra].slice(0, 25).map((o) => {
        const out = { label: String(o.label).slice(0, 100), value: String(o.value).slice(0, 100) };
        if (o.description) out.description = String(o.description).slice(0, 100);
        if (o.emoji) out.emoji = o.emoji;
        return out;
      });
      if (!options.length) throw new Error("A select menu needs at least one option");
      menu.addOptions(options);
      count = options.length;
    }
    const max = Math.max(1, Math.min(count, Math.floor(f.max || 1)));
    menu.setMinValues(Math.max(0, Math.min(max, Math.floor(f.min ?? 1)))).setMaxValues(max);
    return { menu };
  },
};
