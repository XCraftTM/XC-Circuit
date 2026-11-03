const SCOPES = { user: "Users", member: "Members of one server", server: "Servers", channel: "Channels" };
const MENTION = { user: (id) => `<@${id}>`, member: (id) => `<@${id}>`, channel: (id) => `<#${id}>`, server: (id) => id };

module.exports = {
  id: "core.leaderboard",
  kind: "value",
  info: {
    name: "Leaderboard",
    description: "Ranks everyone by a stored number, such as XP or coins: the top ones, and someone's place.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "data",
    aliases: ["Ranking", "Top list"],
    tags: ["top", "rank", "xp", "levels", "economy", "scores"],
  },
  look: {
    icon: "scale",
  },
  pure: false,
  fields: {
    scope: { type: "select", pin: false, default: "user", choices: SCOPES, description: "Whose values are ranked." },
    server: { type: "any", pin: "only", show: { scope: ["member"] }, description: "The server whose members are ranked." },
    key: { type: "text", required: true, default: "xp", description: "The stored value to rank by." },
    top: { type: "number", default: 10, min: 1, max: 100, step: 1, description: "How many places." },
    who: { type: "any", pin: "only", description: "Someone to find the place of (optional)." },
    lowest: { type: "boolean", default: false, advanced: true, label: "Lowest first", description: "Rank small numbers first (such as fastest times)." },
  },
  outputs: {
    entries: { type: "list<object>", description: "The top places: rank, id and value." },
    text: { type: "text", description: "One line per place: 1. @someone · 120" },
    rank: { type: "number", description: "The place of Who (0 when not ranked)." },
    value: { type: "number", description: "Who's value." },
    total: { type: "number", description: "How many are ranked in all." },
  },
  run({ fields: f, ctx }) {
    const all = ctx.store
      .each(f.scope, f.server)
      .map((b) => ({ id: b.id, value: Number(b.values[f.key]) }))
      .filter((e) => Number.isFinite(e.value))
      .sort((a, b) => (f.lowest ? a.value - b.value : b.value - a.value))
      .map((e, i) => ({ rank: i + 1, ...e }));
    const top = all.slice(0, Math.max(1, Math.floor(f.top || 10)));
    const mention = MENTION[f.scope] || ((id) => id);
    const whoId = f.who == null ? null : String(f.who.id ?? f.who);
    const mine = whoId ? all.find((e) => e.id === whoId) : null;
    return {
      entries: top,
      text: top.map((e) => `${e.rank}. ${mention(e.id)} · ${e.value}`).join("\n"),
      rank: mine ? mine.rank : 0,
      value: mine ? mine.value : 0,
      total: all.length,
    };
  },
};
