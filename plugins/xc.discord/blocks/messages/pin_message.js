const ACTIONS = { pin: "Pin", unpin: "Unpin" };

module.exports = {
  id: "discord.pin_message",
  kind: "action",
  info: {
    name: "Pin Message",
    description: "Pins a message to its channel, or unpins it.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "messages",
    aliases: ["Pin", "Unpin"],
    tags: ["unpin", "pinned"],
  },
  look: {
    icon: "message",
  },
  fields: {
    message: { type: "discord.message", pin: "only", required: true, description: "The message." },
    action: { type: "select", pin: false, default: "pin", choices: ACTIONS, description: "Pin or unpin." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  async run({ fields }) {
    if (fields.action === "unpin") await fields.message.unpin(fields.reason || undefined);
    else await fields.message.pin(fields.reason || undefined);
  },
};
