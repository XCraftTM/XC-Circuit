const INFO = {
  name: ["Name", "text", (r) => r.name],
  id: ["Id", "text", (r) => r.id],
  mention: ["Mention", "text", (r) => r.toString()],
  color: ["Color", "text", (r) => r.hexColor],
  members: ["Member count", "number", (r) => r.members.size],
  memberList: ["Members", "list<discord.member>", (r) => [...r.members.values()]],
  position: ["Position", "number", (r) => r.position],
  hoist: ["Shown separately", "boolean", (r) => r.hoist],
  mentionable: ["Mentionable", "boolean", (r) => r.mentionable],
  managed: ["Managed by an app", "boolean", (r) => r.managed],
  permissions: ["Permissions", "list<text>", (r) => r.permissions.toArray()],
  createdAt: ["Created at", "date", (r) => r.createdAt],
};
const CHOICES = Object.fromEntries(Object.entries(INFO).map(([k, [label]]) => [k, label]));

module.exports = {
  id: "discord.role_info",
  name: "Role Info",
  description: "Reads one piece of information about a role. Member lists need the Server Members intent.",
  author: "XC Team",
  version: "1.0.0",
  category: "roles",
  icon: "tag",
  kind: "value",
  fields: {
    role: { type: "discord.role", pin: "only", required: true, description: "The role to read from." },
    info: { type: "select", pin: false, default: "name", choices: CHOICES, description: "What to read." },
  },
  outputs: {
    result: { type: "text", description: "The value you picked." },
  },
  define({ values }) {
    const [label, type] = INFO[values.info] || INFO.name;
    return { outputs: { result: { type, label, description: `The role's ${label.toLowerCase()}.` } } };
  },
  run({ fields }) {
    const [, , read] = INFO[fields.info] || INFO.name;
    return { result: read(fields.role) };
  },
};
