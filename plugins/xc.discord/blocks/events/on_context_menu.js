const TARGETS = { user: "A user (right-click a person)", message: "A message (right-click a message)" };

module.exports = {
  id: "discord.on_context_menu",
  kind: "event",
  info: {
    name: "Context Menu Used",
    description: "Adds an entry to Discord's right-click menu (Apps) for users or messages, and runs when someone uses it.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "commands",
    aliases: ["Right-click", "App command"],
    tags: ["right-click", "apps", "user command", "message command"],
  },
  look: {
    icon: "pointer",
  },
  requires: {
    intents: ["Guilds"],
  },
  fields: {
    name: { type: "text", pin: false, template: false, required: true, default: "Report", description: "The menu entry's name (up to 32 characters, spaces allowed). Registered when the bot starts." },
    target: { type: "select", pin: false, default: "user", choices: TARGETS, description: "What the entry appears on." },
  },
  outputs: {
    interaction: { type: "discord.interaction", description: "The menu use. Connect it to Send Message or Show Modal." },
    user: { type: "discord.user", description: "Who used the entry." },
    member: { type: "discord.member?", description: "Who used it, as a server member. Empty in DMs." },
    channel: { type: "discord.channel", description: "Where it was used." },
    server: { type: "discord.guild?", description: "The server it was used in. Empty in DMs." },
  },
  define({ values }) {
    if (values.target === "message") {
      return { outputs: { message: { type: "discord.message", description: "The message it was used on." }, author: { type: "discord.user", description: "Who wrote that message." } } };
    }
    return { outputs: { targetUser: { type: "discord.user", label: "Target user", description: "The user it was used on." }, targetMember: { type: "discord.member?", label: "Target member", description: "The same, as a server member. Empty in DMs." } } };
  },
  listen(ctx, emit) {
    const name = String(ctx.fields.name || "").trim();
    const onMessage = ctx.fields.target === "message";
    ctx.discord.client.on("interactionCreate", (interaction) => {
      if (!interaction.isContextMenuCommand() || interaction.commandName !== name) return;
      if (onMessage && interaction.isMessageContextMenuCommand()) {
        emit({ interaction, user: interaction.user, member: interaction.member ?? null, channel: interaction.channel, server: interaction.guild ?? null, message: interaction.targetMessage, author: interaction.targetMessage.author });
      } else if (!onMessage && interaction.isUserContextMenuCommand()) {
        emit({ interaction, user: interaction.user, member: interaction.member ?? null, channel: interaction.channel, server: interaction.guild ?? null, targetUser: interaction.targetUser, targetMember: interaction.targetMember ?? null });
      }
    });
  },
};
