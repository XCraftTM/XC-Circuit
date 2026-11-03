module.exports = {
  id: "discord.find_user",
  kind: "action",
  info: {
    name: "Find User",
    description: "Looks up any Discord user by id, also people who share no server with the bot.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    tags: ["fetch user", "lookup", "id"],
  },
  look: {
    icon: "user",
  },
  fields: {
    id: { type: "text", required: true, default: "", label: "User id", description: "The user's id, or a mention." },
  },
  outputs: {
    user: { type: "discord.user", description: "The user. Fails when the id is wrong." },
  },
  async run({ fields, ctx }) {
    const id = String(fields.id?.id ?? fields.id).replace(/[<@!>]/g, "");
    return { user: await ctx.discord.client.users.fetch(id) };
  },
};
