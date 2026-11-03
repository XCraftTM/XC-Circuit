const STATUS = { online: "Online", idle: "Idle", dnd: "Do not disturb", invisible: "Invisible" };

module.exports = {
  id: "fluxer.set_status",
  kind: "action",
  info: {
    name: "Set Bot Status",
    description: "Sets the bot's online status and the custom status text under its name.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "bot",
    aliases: ["Presence", "Activity", "Custom status"],
  },
  look: {
    icon: "settings",
  },
  fields: {
    status: { type: "select", pin: false, default: "online", choices: STATUS, description: "The dot next to the bot's name." },
    text: { type: "text", default: "", label: "Custom status", description: "A short text under the bot's name. Empty = none." },
  },
  run({ fields, ctx }) {
    const user = ctx.fluxer.client.user;
    if (!user) throw new Error("The bot isn't logged in yet: use this after Bot Ready");
    const text = String(fields.text ?? "").trim();
    user.setPresence({ status: fields.status || "online", customStatus: text ? { text } : null });
  },
};
