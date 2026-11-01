module.exports = {
  id: "discord.on_slash_command",
  name: "Slash Command",
  description: "Runs when someone uses one of your slash commands.",
  author: "XC Team",
  version: "1.0.0",
  category: "events",
  icon: "slash",
  kind: "event",
  intents: ["Guilds"],
  fields: {
    command: { type: "text", pin: false, required: true, default: "rank", description: "The command name, without the slash." },
    description: {
      type: "text",
      pin: false,
      advanced: true,
      default: "A command made with XC Circuit",
      description: "Shown under the command in Discord's command list (up to 100 characters).",
    },
  },
  outputs: {
    interaction: { type: "discord.interaction", description: "The command use. Connect it to Reply." },
    user: { type: "discord.user", description: "Who used the command." },
    member: { type: "discord.member?", description: "Who used it, as a server member. Empty in DMs." },
    channel: { type: "discord.channel", description: "Where it was used." },
    server: { type: "discord.guild?", description: "The server it was used in. Empty in DMs." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("interactionCreate", (interaction) => {
      if (!interaction.isChatInputCommand() || interaction.commandName !== ctx.fields.command) return;
      emit({
        interaction,
        user: interaction.user,
        member: interaction.member ?? null,
        channel: interaction.channel,
        server: interaction.guild ?? null,
      });
    });
  },
};
