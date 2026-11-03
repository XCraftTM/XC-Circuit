module.exports = {
  id: "discord.on_user_update",
  kind: "event",
  info: {
    name: "User Changed",
    description: "Runs when someone changes their username, display name or avatar. Only for users the bot shares a server with (needs the Server Members intent).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_members",
    aliases: ["Username changed", "Avatar changed", "Profile changed"],
    tags: ["user", "username", "avatar", "rename", "log"],
  },
  look: {
    icon: "user",
  },
  requires: {
    intents: ["Guilds", "GuildMembers"],
  },
  fields: {},
  outputs: {
    user: { type: "discord.user", description: "The user, as they are now." },
    beforeName: { type: "text", label: "Username before", description: "Their username before." },
    beforeDisplayName: { type: "text", label: "Display name before", description: "Their display name before." },
    beforeAvatar: { type: "text", label: "Avatar before", description: "Their avatar's address before." },
    changed: { type: "list<text>", description: "What changed: username, displayName, avatar." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("userUpdate", (old, now) => {
      const changed = [];
      if (old.username !== now.username) changed.push("username");
      if (old.globalName !== now.globalName) changed.push("displayName");
      if (old.avatar !== now.avatar) changed.push("avatar");
      if (!changed.length) return;
      emit({ user: now, beforeName: old.username ?? "", beforeDisplayName: old.globalName ?? old.username ?? "", beforeAvatar: old.displayAvatarURL?.() ?? "", changed });
    });
  },
};
