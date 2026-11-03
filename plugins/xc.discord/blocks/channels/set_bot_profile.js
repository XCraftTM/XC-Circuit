module.exports = {
  id: "discord.set_bot_profile",
  kind: "action",
  info: {
    name: "Set Bot Profile",
    description: "Changes the bot's username, avatar or banner. Discord allows only a few changes per hour.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    tags: ["avatar", "rename bot", "username"],
  },
  look: {
    icon: "bolt",
  },
  fields: {
    username: { type: "text", default: "", description: "New username. Empty keeps it." },
    avatar: { type: "text", default: "", label: "Avatar URL", description: "New avatar image. Empty keeps it." },
    banner: { type: "text", default: "", label: "Banner URL", advanced: true, description: "New profile banner. Empty keeps it." },
  },
  outputs: {
    user: { type: "discord.user", description: "The bot's user." },
  },
  async run({ fields: f, ctx }) {
    const me = ctx.discord.client.user;
    const edit = {};
    if (f.username) edit.username = String(f.username).slice(0, 32);
    if (f.avatar) edit.avatar = f.avatar;
    if (f.banner) edit.banner = f.banner;
    return { user: Object.keys(edit).length ? await me.edit(edit) : me };
  },
};
