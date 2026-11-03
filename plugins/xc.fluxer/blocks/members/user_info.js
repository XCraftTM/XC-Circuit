module.exports = {
  id: "fluxer.user_info",
  kind: "action",
  info: {
    name: "User Info",
    description: "An account's name, id, picture and age. For a member, connect it: it turns into its user.",
    author: "XC Team",
    version: "2.0.0",
  },
  search: {
    category: "members",
    aliases: ["Username", "User id", "Avatar", "Account age"],
  },
  look: {
    icon: "user",
  },
  fields: {
    user: { type: "fluxer.user", pin: "only", required: true, description: "The user." },
  },
  outputs: {
    username: { type: "text", description: "Their unique username." },
    name: { type: "text", label: "Display name", description: "Their display name, else their username." },
    id: { type: "text", description: "Their id." },
    avatar: { type: "text", description: "Their picture's web address." },
    bot: { type: "boolean", label: "Is a bot", description: "Whether the account is a bot." },
    createdAt: { type: "date", label: "Created at", description: "When the account was made." },
    mention: { type: "text", description: "<@id>, which pings them in a message." },
  },
  run({ fields }) {
    const u = fields.user?.user ?? fields.user;
    if (!u) throw new Error("Connect the user");
    return {
      username: u.username ?? "",
      name: u.globalName || u.username || "",
      id: u.id,
      avatar: u.displayAvatarURL?.() ?? "",
      bot: Boolean(u.bot),
      createdAt: u.createdAt ?? null,
      mention: `<@${u.id}>`,
    };
  },
};
