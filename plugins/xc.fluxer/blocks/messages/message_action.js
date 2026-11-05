const ACTIONS = {
  delete: "Delete it",
  react: "Add a reaction",
  unreact: "Remove the bot's reaction",
  clearReactions: "Remove all reactions",
  pin: "Pin it",
  unpin: "Unpin it",
};

module.exports = {
  id: "fluxer.message_action",
  kind: "action",
  info: {
    name: "Change Message",
    description: "Deletes, pins or unpins a message, or adds and removes reactions.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "messages",
    aliases: ["Delete message", "Add reaction", "React", "Pin message", "Unpin"],
  },
  look: {
    icon: "message",
  },
  requires: {
    permissions: {
      action: {
        delete: ["ManageMessages"],
        react: ["AddReactions", "ReadMessageHistory"],
        clearReactions: ["ManageMessages"],
        pin: ["PinMessages"],
        unpin: ["PinMessages"],
      },
    },
  },
  fields: {
    message: { type: "fluxer.message", pin: "only", required: true, description: "The message." },
    action: { type: "select", pin: false, default: "delete", choices: ACTIONS, description: "What to do with it." },
    emoji: {
      type: "text",
      default: "👍",
      show: { action: ["react", "unreact"] },
      description: "An emoji such as 👍, or a custom one as <:name:id>.",
    },
    reason: { type: "text", default: "", advanced: true, show: { action: ["delete"] }, description: "Why, for the server's audit log." },
  },
  async run({ fields }) {
    const m = fields.message;
    if (!m) throw new Error("Connect the message");
    const emoji = String(fields.emoji ?? "").trim();
    if (["react", "unreact"].includes(fields.action) && !emoji) throw new Error("Type the emoji");
    switch (fields.action) {
      case "react":
        return void (await m.react(emoji));
      case "unreact":
        return void (await m.removeReaction(emoji));
      case "clearReactions":
        return void (await m.removeAllReactions());
      case "pin":
        return void (await m.pin());
      case "unpin":
        return void (await m.unpin());
      default:
        await m.delete(fields.reason || undefined);
    }
  },
};
