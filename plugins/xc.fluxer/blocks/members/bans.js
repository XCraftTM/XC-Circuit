const ACTIONS = { list: "List every ban", check: "Check if someone is banned" };

module.exports = {
  id: "fluxer.bans",
  kind: "action",
  info: {
    name: "Bans",
    description: "Lists a server's bans with their reasons, or checks whether someone is banned.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["Ban list", "Is banned"],
  },
  look: {
    icon: "shield",
  },
  requires: {
    permissions: ["BanMembers"],
  },
  fields: {
    action: { type: "select", pin: false, default: "list", choices: ACTIONS, description: "What to do." },
    server: { type: "fluxer.server", pin: "only", required: true, description: "The server." },
    who: { type: "any", pin: true, show: { action: ["check"] }, description: "A user, member or id." },
  },
  outputs: {
    bans: { type: "list<object>", description: "For lists: each ban's user, reason and end (until)." },
    count: { type: "number", description: "For lists: how many bans." },
    banned: { type: "boolean", description: "For checks: whether they are banned." },
    reason: { type: "text", description: "For checks: why, if banned." },
  },
  async run({ fields: f, ctx }) {
    if (!f.server) throw new Error("Connect the server");
    const bans = (await f.server.fetchBans()).map((b) => ({ user: b.user, userId: b.user?.id, reason: b.reason ?? "", until: b.expiresAt ?? null }));
    if (f.action !== "check") return { bans, count: bans.length, banned: false, reason: "" };
    const userId = ctx.fluxer.id(f.who, "user");
    if (!userId) throw new Error("Connect who, or type their id");
    const ban = bans.find((b) => String(b.userId) === userId);
    return { bans: [], count: 0, banned: Boolean(ban), reason: ban?.reason ?? "" };
  },
};
