const INFO = {
  username: ["Username", "text", (u) => u.username],
  displayName: ["Display name", "text", (u) => u.displayName],
  id: ["Id", "text", (u) => u.id],
  mention: ["Mention", "text", (u) => u.toString()],
  avatar: ["Avatar URL", "text", (u) => u.displayAvatarURL({ size: 512 })],
  createdAt: ["Account created", "date", (u) => u.createdAt],
  bot: ["Is a bot", "boolean", (u) => u.bot],
  globalName: ["Global name", "text", (u) => u.globalName ?? ""],
  tag: ["Tag", "text", (u) => u.tag],
  defaultAvatar: ["Default avatar URL", "text", (u) => u.defaultAvatarURL],
  banner: ["Banner URL", "text", async (u) => (await u.fetch(true)).bannerURL({ size: 1024 }) ?? ""],
  accentColor: ["Profile color", "text", async (u) => (await u.fetch(true)).hexAccentColor ?? ""],
  system: ["Is Discord itself", "boolean", (u) => Boolean(u.system)],
  dm: ["DM channel", "discord.channel", (u) => u.createDM()],
};

module.exports = {
  id: "discord.user_info",
  kind: "value",
  info: {
    name: "User Info",
    description: "Reads one piece of information about a user (works for members too).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    aliases: ["Avatar", "Username"],
  },
  look: {
    icon: "user",
  },
  fields: {
    user: { type: "discord.user", pin: "only", required: true, description: "A user or member." },
    info: { type: "select", pin: false, default: "username", choices: Object.fromEntries(Object.entries(INFO).map(([k, [l]]) => [k, l])), description: "Which detail about the user to get." },
  },
  outputs: {
    result: { type: "text", description: "The detail you picked." },
  },
  define({ values }) {
    const [label, type] = INFO[values.info] || INFO.username;
    return { outputs: { result: { type, label, description: `The user's ${label.toLowerCase()}.` } } };
  },
  async run({ fields }) {
    const user = fields.user?.user ?? fields.user;
    const [, , read] = INFO[fields.info] || INFO.username;
    return { result: await read(user) };
  },
};
