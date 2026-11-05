module.exports = {
  id: "discord.add_reaction",
  kind: "action",
  info: {
    name: "Add Reaction",
    description: "Reacts to a message with an emoji.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "reactions",
    aliases: ["React", "Emoji reaction"],
    tags: ["emoji", "react"],
  },
  look: {
    icon: "message",
  },
  requires: {
    permissions: ["AddReactions", "ReadMessageHistory"],
  },
  fields: {
    message: { type: "discord.message", pin: "only", required: true, description: "The message to react to." },
    emoji: { type: "text", required: true, default: "👍", description: "An emoji such as 👍 or <:name:id>." },
  },
  async run({ fields }) {
    await fields.message.react(fields.emoji);
  },
};
