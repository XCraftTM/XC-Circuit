const commandsOf = (project) => (project && project.data && project.data.slashCommands) || [];

module.exports = {
  id: "discord.on_autocomplete",
  kind: "event",
  info: {
    name: "Autocomplete",
    description: "Runs while someone types into a slash command option that has Autocomplete on. Answer with Respond to Autocomplete.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_commands",
    aliases: ["Suggestions"],
    tags: ["suggest", "search", "options"],
  },
  look: {
    icon: "slash",
  },
  requires: {
    intents: ["Guilds"],
  },
  fields: {
    command: { type: "slash-command-ref", pin: false, required: true, default: "", description: "Which command's options this suggests for." },
    option: { type: "text", pin: false, template: false, default: "", description: "Only this option. Empty = any of the command's autocomplete options." },
  },
  outputs: {
    interaction: { type: "discord.interaction", description: "Connect it to Respond to Autocomplete." },
    option: { type: "text", description: "The option being typed into." },
    typed: { type: "text", description: "What has been typed so far." },
    user: { type: "discord.user", description: "Who is typing." },
  },
  define({ values, project }) {
    const choices = Object.fromEntries(commandsOf(project).map((c) => [c.name, `/${c.name}`]));
    if (values.command && !(values.command in choices)) choices[values.command] = `/${values.command} (not in the builder)`;
    return { set: { command: { choices } } };
  },
  listen(ctx, emit) {
    const name = String(ctx.fields.command || "").toLowerCase();
    const only = String(ctx.fields.option || "").trim();
    ctx.discord.client.on("interactionCreate", (interaction) => {
      if (!interaction.isAutocomplete() || interaction.commandName !== name) return;
      const focused = interaction.options.getFocused(true);
      if (only && focused.name !== only) return;
      emit({ interaction, option: focused.name, typed: String(focused.value ?? ""), user: interaction.user });
    });
  },
};
