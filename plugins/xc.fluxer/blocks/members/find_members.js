module.exports = {
  id: "fluxer.find_members",
  kind: "action",
  info: {
    name: "Find Members",
    description: "Searches a server's members by name, role or whether they're bots: for member lists, counts and picking winners.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    aliases: ["List members", "Search members", "Members with role", "Member count"],
  },
  look: {
    icon: "user",
  },
  fields: {
    server: { type: "fluxer.server", pin: "only", required: true, description: "The server." },
    name: { type: "text", default: "", description: "Only names containing this. Empty = everyone." },
    role: { type: "any", pin: true, description: "Only members with this role (a role or its id)." },
    bots: { type: "select", pin: false, default: "any", choices: { any: "People and bots", people: "Only people", bots: "Only bots" }, description: "Who counts." },
    limit: { type: "number", default: 100, min: 1, max: 1000, step: 1, description: "At most this many." },
  },
  outputs: {
    members: { type: "list<fluxer.member>", description: "The matching members (when Fluxer gives them)." },
    found: { type: "list<object>", description: "Each match: userId, username, globalName and nickname." },
    ids: { type: "list<text>", label: "User ids", description: "Their user ids." },
    count: { type: "number", description: "How many matched (in all, not just the ones given)." },
  },
  async run({ fields: f, ctx }) {
    if (!f.server) throw new Error("Connect the server");
    const roleId = f.role ? ctx.fluxer.id(f.role, "role") : null;
    const options = { limit: Math.min(1000, Math.max(1, Math.floor(f.limit || 100))) };
    const name = String(f.name ?? "").trim();
    if (name) options.query = name;
    if (roleId) options.roleIds = [roleId];
    if (f.bots !== "any") options.isBot = f.bots === "bots";
    const result = await f.server.members.search(options);
    const hits = result?.members ?? [];
    const found = hits.map((m) => ({ userId: m.userId, username: m.username, globalName: m.globalName ?? null, nickname: m.nickname ?? null }));
    return { members: hits.map((m) => m.member).filter(Boolean), found, ids: found.map((m) => String(m.userId)), count: Number(result?.total ?? found.length) };
  },
};
