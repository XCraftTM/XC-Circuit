const ACTIONS = { check: "Check if someone is banned", list: "List the bans", bulk: "Ban many at once" };

/** User ids from connected users/members and typed lines. */
function idsOf(users, text) {
  const fromPins = [users].flat().filter(Boolean).map((u) => u.id ?? String(u));
  const typed = String(text || "").match(/\d{17,20}/g) || [];
  return [...new Set([...fromPins, ...typed])];
}

module.exports = {
  id: "discord.bans",
  kind: "action",
  info: {
    name: "Bans",
    description: "Checks whether someone is banned (and why), lists a server's bans, or bans up to 200 users at once (for raids). Banning one member is in Moderate Member. The bot needs Ban Members.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["Ban list", "Is banned", "Mass ban", "Bulk ban"],
    tags: ["ban", "bans", "raid", "moderation", "unban"],
  },
  look: {
    icon: "shield",
  },
  fields: {
    action: { type: "select", pin: false, default: "check", choices: ACTIONS, description: "What to do." },
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    user: { type: "discord.user?", pin: "only", show: { action: ["check"] }, description: "Who to check." },
    userId: { type: "text", default: "", show: { action: ["check"] }, label: "Or user id", description: "Their id instead (banned people aren't members)." },
    users: { type: "list<discord.user>", pin: "only", multi: true, show: { action: ["bulk"] }, description: "Who to ban." },
    ids: { type: "textarea", default: "", show: { action: ["bulk"] }, label: "Or user ids", description: "Ids to ban too, one per line or separated by spaces." },
    deleteHours: { type: "number", default: 0, min: 0, max: 168, show: { action: ["bulk"] }, label: "Delete messages (hours)", description: "Also delete their messages from this many past hours (up to 7 days)." },
    reason: { type: "text", template: true, default: "", show: { action: ["bulk"] }, description: "Shown in the audit log." },
  },
  outputs: {
    banned: { type: "boolean", description: "For check: whether they are banned." },
    reason: { type: "text", description: "For check: the ban's reason." },
    list: { type: "list<object>", description: "For lists: each user (id, username) with the reason." },
    bannedIds: { type: "list<text>", label: "Banned ids", description: "For many: who was banned." },
    failedIds: { type: "list<text>", label: "Failed ids", description: "For many: who couldn't be banned." },
  },
  async run({ fields: f }) {
    const s = f.server;
    if (!s?.bans) throw new Error("Connect a server");
    const none = { banned: false, reason: "", list: [], bannedIds: [], failedIds: [] };
    if (f.action === "check") {
      const id = f.user?.id ?? String(f.userId || "").trim();
      if (!id) throw new Error("Connect a user or give their id");
      const ban = await s.bans.fetch({ user: id, force: true }).catch((e) => (e.code === 10026 ? null : Promise.reject(e)));
      return { ...none, banned: Boolean(ban), reason: ban?.reason ?? "" };
    }
    if (f.action === "list") {
      const all = await s.bans.fetch();
      return { ...none, list: [...all.values()].map((b) => ({ id: b.user.id, username: b.user.username, reason: b.reason ?? "" })) };
    }
    const ids = idsOf(f.users, f.ids);
    if (!ids.length) throw new Error("Connect users or give their ids");
    if (ids.length > 200) throw new Error("Discord bans at most 200 at once");
    const result = await s.bans.bulkCreate(ids, { deleteMessageSeconds: Math.round((Number(f.deleteHours) || 0) * 3600), reason: f.reason || undefined });
    return { ...none, bannedIds: [...result.bannedUsers], failedIds: [...result.failedUsers] };
  },
};
