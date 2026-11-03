const ACTIONS = { add: "Add a warning", list: "List warnings", remove: "Remove the latest", clear: "Clear all" };

module.exports = {
  id: "discord.warnings",
  kind: "action",
  info: {
    name: "Warnings",
    description: "Keeps warnings for members in stored data: add one, list them, remove the latest or clear them.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["Warn", "Strikes"],
    tags: ["warn", "strike", "moderation", "infractions"],
  },
  look: {
    icon: "shield",
  },
  fields: {
    action: { type: "select", pin: false, default: "add", choices: ACTIONS, description: "What to do." },
    member: { type: "discord.member", pin: "only", required: true, description: "The member." },
    reason: { type: "text", default: "", show: { action: ["add"] }, description: "Why." },
    by: { type: "discord.user?", pin: "only", show: { action: ["add"] }, label: "Given by", description: "The moderator." },
  },
  outputs: {
    warnings: { type: "list<object>", description: "Their warnings: reason, by (user id) and at (date)." },
    count: { type: "number", description: "How many they have now." },
    text: { type: "text", description: "One line per warning." },
  },
  async run({ fields: f, ctx }) {
    const store = ctx.store.member(f.member.guild.id, f.member.id);
    let list = await store.get("warnings", []);
    if (f.action === "add") list = [...list, { reason: String(f.reason || ""), by: f.by?.id ?? "", at: new Date().toISOString() }];
    if (f.action === "remove") list = list.slice(0, -1);
    if (f.action === "clear") list = [];
    if (f.action !== "list") await store.set("warnings", list);
    const text = list.map((w, i) => `${i + 1}. ${w.reason || "No reason"}${w.by ? ` (by <@${w.by}>)` : ""} · <t:${Math.floor(new Date(w.at).getTime() / 1000)}:d>`).join("\n");
    return { warnings: list, count: list.length, text };
  },
};
