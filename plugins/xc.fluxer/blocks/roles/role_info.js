const INFO = {
  name: "Name",
  id: "Id",
  mention: "Mention (<@&id>)",
  color: "Color (#RRGGBB)",
  position: "Position",
  hoist: "Shown separately",
  mentionable: "Mentionable",
  emoji: "Emoji",
};

module.exports = {
  id: "fluxer.role_info",
  kind: "value",
  info: {
    name: "Role Info",
    description: "One thing about a role: its name, color, position and more.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "roles",
    aliases: ["Role name", "Role color"],
  },
  look: {
    icon: "tag",
  },
  fields: {
    role: { type: "fluxer.role", pin: "only", required: true, description: "The role." },
    info: { type: "select", pin: false, default: "name", choices: INFO, label: "What", description: "What to give." },
  },
  outputs: {
    value: { type: "any", description: "The answer." },
  },
  run({ fields: f }) {
    const r = f.role;
    if (!r) throw new Error("Connect the role");
    const value = {
      name: () => r.name,
      id: () => r.id,
      mention: () => `<@&${r.id}>`,
      color: () => `#${Number(r.color || 0).toString(16).padStart(6, "0")}`,
      position: () => r.position ?? 0,
      hoist: () => Boolean(r.hoist),
      mentionable: () => Boolean(r.mentionable),
      emoji: () => r.unicodeEmoji ?? "",
    }[f.info];
    return { value: value ? value() : null };
  },
};
