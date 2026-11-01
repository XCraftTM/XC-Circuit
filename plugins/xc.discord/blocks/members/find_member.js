module.exports = {
  id: "discord.find_member",
  name: "Find Member",
  description: "Looks up a server member by id. Use it to turn a User into a Member.",
  author: "XC Team",
  version: "1.0.0",
  category: "members",
  icon: "user",
  kind: "action",
  tags: ["fetch", "get", "lookup"],
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server to look in." },
    user: { type: "any", pin: true, required: true, description: "A user, or a user id." },
  },
  outputs: {
    member: { type: "discord.member", description: "The member. Fails when they are not in the server." },
  },
  async run({ fields }) {
    const id = String(fields.user?.id ?? fields.user);
    return { member: await fields.server.members.fetch(id) };
  },
};
