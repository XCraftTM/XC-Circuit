module.exports = {
  id: "fluxer.set_nickname",
  kind: "action",
  info: {
    name: "Set Nickname",
    description: "Changes a member's nickname in the server, or removes it.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    aliases: ["Nickname", "Rename member"],
  },
  look: {
    icon: "user",
  },
  fields: {
    member: { type: "fluxer.member", pin: "only", required: true, description: "Who." },
    nickname: { type: "text", default: "", description: "The new nickname (up to 32 characters). Empty removes it." },
  },
  async run({ fields }) {
    if (!fields.member?.edit) throw new Error("Connect a member");
    const nick = String(fields.nickname ?? "").trim().slice(0, 32);
    await fields.member.edit({ nick: nick || null });
  },
};
