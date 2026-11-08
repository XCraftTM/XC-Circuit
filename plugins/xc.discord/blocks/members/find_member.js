module.exports = {
  id: "discord.find_member",
  kind: "action",
  info: {
    name: "Find Member",
    description: "Looks up a server member by id or name. Use it to turn a User into a Member.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "members",
    aliases: ["Fetch member", "Get member"],
    tags: ["fetch", "get", "lookup"],
  },
  look: {
    icon: "user",
  },
  flows: ["next", "missing"],
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server to look in." },
    by: { type: "select", pin: false, default: "id", choices: { id: "User or id", name: "Name (search)" }, description: "How to find them." },
    user: { type: "any", pin: true, required: true, show: { by: ["id"] }, description: "A user, or a user id." },
    name: { type: "text", default: "", show: { by: ["name"] }, description: "The start of their username or nickname." },
  },
  outputs: {
    member: { type: "discord.member", description: "The member. When they aren't in the server, Missing runs instead (or, with nothing connected to Missing, the block fails)." },
  },
  async run({ fields, ctx }) {
    let member = null;
    let why;
    if (fields.by === "name") {
      const found = await fields.server.members.search({ query: String(fields.name), limit: 1 });
      member = found.size ? found.first() : null;
      why = `Nobody named "${fields.name}" in ${fields.server.name}`;
    } else {
      const id = String(fields.user?.id ?? fields.user).trim().replace(/^<@!?(\d+)>$/, "$1");
      member = await fields.server.members.fetch(id).catch((e) => {
        why = `${id} isn't in ${fields.server.name} (${e.message})`;
        return null;
      });
    }
    if (!member) {
      if (ctx.wired?.("missing")) return ctx.next("missing", { member: null });
      throw new Error(why);
    }
    await ctx.next("next", { member });
  },
};
