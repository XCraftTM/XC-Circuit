const KINDS = { user: "Person", role: "Role", channel: "Channel" };

module.exports = {
  id: "fluxer.mention",
  kind: "value",
  info: {
    name: "Mention",
    description: "Text that pings a person or role, or links a channel, in a message: <@id>, <@&id>, <#id>.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "format",
    aliases: ["Ping", "Tag", "@"],
  },
  look: {
    icon: "text",
  },
  fields: {
    target: { type: "any", pin: true, description: "A user, member, role or channel, or an id." },
    kind: { type: "select", pin: false, default: "user", choices: KINDS, label: "An id is a", description: "What a typed id stands for. Connected things know it themselves." },
  },
  outputs: {
    text: { type: "text", description: "The mention." },
  },
  run({ fields }) {
    const t = fields.target;
    let kind = fields.kind || "user";
    if (t && typeof t === "object") {
      if (typeof t.isTextBased === "function") kind = "channel";
      else if (t.username !== undefined || t.user) kind = "user";
      else if (t.position !== undefined && t.guildId !== undefined) kind = "role";
    }
    const id = t && typeof t === "object" ? t.id : String(t ?? "").replace(/\D/g, "");
    if (!id) throw new Error("Connect who or what to mention, or type an id");
    return { text: kind === "channel" ? `<#${id}>` : kind === "role" ? `<@&${id}>` : `<@${id}>` };
  },
};
