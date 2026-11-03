module.exports = {
  id: "discord.find_member",
  kind: "action",
  info: {
    name: "Find Member",
    description: "Looks up a server member by id or name. Use it to turn a User into a Member.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    aliases: ["Fetch member", "Get member"],
    tags: ["fetch", "get", "lookup"],
  },
  look: {
    icon: "user",
  },
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server to look in." },
    by: { type: "select", pin: false, default: "id", choices: { id: "User or id", name: "Name (search)" }, description: "How to find them." },
    user: { type: "any", pin: true, required: true, show: { by: ["id"] }, description: "A user, or a user id." },
    name: { type: "text", default: "", show: { by: ["name"] }, description: "The start of their username or nickname." },
  },
  outputs: {
    member: { type: "discord.member", description: "The member. Fails when they are not in the server." },
  },
  async run({ fields }) {
    if (fields.by === "name") {
      const found = await fields.server.members.search({ query: String(fields.name), limit: 1 });
      if (!found.size) throw new Error(`Nobody named "${fields.name}" in ${fields.server.name}`);
      return { member: found.first() };
    }
    const id = String(fields.user?.id ?? fields.user);
    return { member: await fields.server.members.fetch(id) };
  },
};
