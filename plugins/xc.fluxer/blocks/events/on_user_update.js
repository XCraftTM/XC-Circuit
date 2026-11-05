module.exports = {
  id: "fluxer.on_user_update",
  kind: "event",
  info: {
    name: "User Changed",
    description: "Runs when someone changes their username, display name or avatar. Only for users the bot shares a server with.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    aliases: ["On user update", "Username changed", "Avatar changed"],
  },
  look: {
    icon: "user",
  },
  fields: {},
  outputs: {
    user: { type: "fluxer.user", description: "The user, as they are now." },
    username: { type: "text", description: "Their username now." },
    avatar: { type: "text", description: "Their avatar's link now." },
  },
  listen(ctx, emit) {
    ctx.fluxer.client.on("userUpdate", (user) => emit({ user, username: user?.username ?? "", avatar: user?.displayAvatarURL?.() ?? "" }));
  },
};
