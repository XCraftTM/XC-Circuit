module.exports = {
  id: "fluxer.find_member",
  kind: "action",
  info: {
    name: "Find Member",
    description: "Finds a server member by their id or mention, such as the @someone in !warn @someone.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    aliases: ["Get member", "Member from mention", "Fetch member"],
  },
  look: {
    icon: "user",
  },
  fields: {
    server: { type: "fluxer.server", pin: "only", required: true, description: "The server." },
    who: { type: "text", default: "", description: "Their id or mention (<@id>)." },
  },
  outputs: {
    member: { type: "fluxer.member?", description: "The member. Empty when not found." },
    found: { type: "boolean", description: "Whether they are in the server." },
  },
  async run({ fields, ctx }) {
    if (!fields.server) throw new Error("Connect the server");
    const userId = ctx.fluxer.id(fields.who, "user");
    if (!userId) return { member: null, found: false };
    const member = await fields.server.fetchMember(userId).catch(() => null);
    return { member, found: Boolean(member) };
  },
};
