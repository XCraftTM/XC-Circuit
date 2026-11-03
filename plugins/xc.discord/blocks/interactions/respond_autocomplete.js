module.exports = {
  id: "discord.respond_autocomplete",
  kind: "action",
  info: {
    name: "Respond to Autocomplete",
    description: "Shows suggestions while someone types into an autocomplete option. Up to 25.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "components",
    aliases: ["Send suggestions"],
    tags: ["suggest", "search"],
  },
  look: {
    icon: "slash",
  },
  fields: {
    interaction: { type: "discord.interaction", pin: "only", required: true, description: "From the Autocomplete event." },
    suggestions: { type: "textarea", default: "", description: "Suggestions, one per line: value | Label, or only a label." },
    items: { type: "list<any>", pin: "only", multi: true, description: "More suggestions from a list: texts, or objects with name (or label) and value." },
    filter: { type: "boolean", default: true, label: "Only matching", description: "Keep only suggestions containing what was typed." },
  },
  async run({ fields: f }) {
    const typed = String(f.interaction.options.getFocused() ?? "").toLowerCase();
    const lines = String(f.suggestions || "").split(/\r?\n/).map((l) => l.trim()).filter(Boolean).map((l) => {
      const [value, label] = l.split("|").map((s) => s.trim());
      return { name: label || value, value };
    });
    const extra = (f.items || []).flat().filter((x) => x != null).map((x) => (typeof x === "object" ? { name: String(x.name ?? x.label ?? x.value), value: String(x.value ?? x.name ?? x.label) } : { name: String(x), value: String(x) }));
    let all = [...lines, ...extra];
    if (f.filter && typed) all = all.filter((c) => c.name.toLowerCase().includes(typed) || c.value.toLowerCase().includes(typed));
    await f.interaction.respond(all.slice(0, 25).map((c) => ({ name: c.name.slice(0, 100), value: c.value.slice(0, 100) })));
  },
};
