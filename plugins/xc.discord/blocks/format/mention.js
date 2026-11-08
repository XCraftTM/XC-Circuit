const KINDS = { auto: "Detect it", user: "User", role: "Role", channel: "Channel", everyone: "@everyone", here: "@here" };

module.exports = {
  id: "discord.mention",
  kind: "value",
  info: {
    name: "Mention",
    description: "The text that pings someone, a role, @everyone or @here, or links a channel: <@id>, <@&id>, <#id>. A list mentions each. In a text, ${user} (or a role or channel) does the same.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "format",
    aliases: ["Ping user", "@user", "Ping role", "@everyone", "@here"],
    tags: ["ping", "tag", "@", "link channel"],
  },
  look: {
    icon: "user",
  },
  fields: {
    target: { type: "any", pin: true, default: "", description: "A user, member, role or channel, a list of them, or an id (typed or connected). Not needed for @everyone and @here." },
    kind: { type: "select", pin: false, default: "auto", choices: KINDS, description: "What it is, when only an id is given; or @everyone or @here." },
  },
  outputs: {
    text: { type: "text", description: "The mention." },
  },
  run({ fields: f }) {
    if (f.kind === "everyone" || f.kind === "here") return { text: `@${f.kind}` };
    const one = (t) => {
      const id = String(t?.id ?? t ?? "").trim().replace(/^<[@#&!]*|>$/g, "");
      if (!id) return "";
      let kind = f.kind;
      if (kind === "auto") {
        if (t && typeof t === "object" && "hexColor" in t) kind = "role";
        else if (t && typeof t.isTextBased === "function") kind = "channel";
        else kind = "user";
      }
      return kind === "role" ? `<@&${id}>` : kind === "channel" ? `<#${id}>` : `<@${id}>`;
    };
    const targets = Array.isArray(f.target) ? f.target.flat() : [f.target];
    const text = targets.map(one).filter(Boolean).join(" ");
    if (!text) throw new Error("Mention needs a user, role or channel (or an id), or @everyone or @here");
    return { text };
  },
};
