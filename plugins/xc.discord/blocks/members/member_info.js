// Each choice: label, output type, and how to read it from a GuildMember.
const INFO = {
  displayName: ["Display name", "text", (m) => m.displayName],
  id: ["Id", "text", (m) => m.id],
  mention: ["Mention", "text", (m) => m.toString()],
  avatar: ["Avatar URL", "text", (m) => m.displayAvatarURL()],
  joinedAt: ["Joined at", "date", (m) => m.joinedAt],
  roles: ["Roles", "list<discord.role>", (m) => [...m.roles.cache.values()]],
  highestRole: ["Highest role", "discord.role", (m) => m.roles.highest],
  user: ["User", "discord.user", (m) => m.user],
  isTimedOut: ["Is timed out", "boolean", (m) => m.isCommunicationDisabled()],
  kickable: ["Bot can kick", "boolean", (m) => m.kickable],
  bannable: ["Bot can ban", "boolean", (m) => m.bannable],
  nickname: ["Nickname", "text", (m) => m.nickname ?? ""],
  color: ["Name color", "text", (m) => m.displayHexColor],
  permissions: ["Permissions", "list<text>", (m) => m.permissions.toArray()],
  voiceChannel: ["Voice channel", "discord.channel?", (m) => m.voice.channel],
  boostingSince: ["Boosting since", "date", (m) => m.premiumSince],
  timedOutUntil: ["Timed out until", "date", (m) => m.communicationDisabledUntil],
  isOwner: ["Is the server owner", "boolean", (m) => m.id === m.guild.ownerId],
  pending: ["Hasn't passed screening", "boolean", (m) => Boolean(m.pending)],
  serverAvatar: ["Server avatar URL", "text", (m) => m.avatarURL({ size: 512 }) ?? m.displayAvatarURL({ size: 512 })],
  server: ["Server", "discord.guild", (m) => m.guild],
  manageable: ["Bot can change them", "boolean", (m) => m.manageable],
  moderatable: ["Bot can time out", "boolean", (m) => m.moderatable],
};

const CHOICES = Object.fromEntries(Object.entries(INFO).map(([k, [label]]) => [k, label]));

module.exports = {
  id: "discord.member_info",
  kind: "value",
  info: {
    name: "Member Info",
    description: "Reads one piece of information about a server member.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    aliases: ["Member details", "Joined at"],
  },
  look: {
    icon: "user",
  },
  fields: {
    member: { type: "discord.member", pin: "only", required: true, description: "The member to read from." },
    info: { type: "select", pin: false, default: "displayName", choices: CHOICES, description: "What to read." },
  },
  outputs: {
    result: { type: "text", description: "The value you picked." },
  },
  define({ values }) {
    // The output type follows the choice: Text, Date, List of Role …
    const [label, type] = INFO[values.info] || INFO.displayName;
    return { outputs: { result: { type, label, description: `The member's ${label.toLowerCase()}.` } } };
  },
  run({ fields }) {
    const [, , read] = INFO[fields.info] || INFO.displayName;
    return { result: read(fields.member) };
  },
};
