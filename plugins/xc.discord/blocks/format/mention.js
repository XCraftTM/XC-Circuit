const KINDS = { auto: "Detect it", user: "User", role: "Role", channel: "Channel" };

module.exports = {
  id: "discord.mention",
  kind: "value",
  info: {
    name: "Mention",
    description: "The text that pings someone or links a role or channel: <@id>, <@&id>, <#id>.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "format",
    aliases: ["Ping user", "@user"],
    tags: ["ping", "tag", "@", "link channel"],
  },
  look: {
    icon: "user",
  },
  fields: {
    target: { type: "any", pin: "only", required: true, description: "A user, member, role or channel (or an id)." },
    kind: { type: "select", pin: false, default: "auto", choices: KINDS, description: "What it is, when only an id is connected." },
  },
  outputs: {
    text: { type: "text", description: "The mention." },
  },
  run({ fields: f }) {
    const t = f.target;
    const id = String(t?.id ?? t ?? "");
    let kind = f.kind;
    if (kind === "auto") {
      if (t && typeof t === "object" && "hexColor" in t) kind = "role";
      else if (t && typeof t.isTextBased === "function") kind = "channel";
      else kind = "user";
    }
    return { text: kind === "role" ? `<@&${id}>` : kind === "channel" ? `<#${id}>` : `<@${id}>` };
  },
};
