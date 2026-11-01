const MATCH = { equals: "Equals", starts: "Starts with", pattern: "Matches pattern" };

module.exports = {
  id: "discord.on_button",
  name: "Button Pressed",
  description: "Runs when someone presses a button whose custom id matches.",
  author: "XC Team",
  version: "1.0.0",
  category: "events",
  icon: "pointer",
  kind: "event",
  intents: ["Guilds"],
  fields: {
    customId: { type: "text", pin: false, required: true, default: "", label: "Custom id", description: "The id given to the button when it was built." },
    match: { type: "select", pin: false, default: "equals", choices: MATCH, description: "How the id is compared." },
  },
  outputs: {
    interaction: { type: "discord.interaction", description: "The button press. Connect it to Reply." },
    user: { type: "discord.user", description: "Who pressed it." },
    customId: { type: "text", label: "Custom id", description: "The full id of the pressed button." },
    message: { type: "discord.message", description: "The message the button is on." },
  },
  listen(ctx, emit) {
    const { customId: wanted, match } = ctx.fields;
    const test = {
      equals: (id) => id === wanted,
      starts: (id) => id.startsWith(wanted),
      pattern: (id) => new RegExp(wanted).test(id),
    }[match];
    ctx.discord.client.on("interactionCreate", (interaction) => {
      if (!interaction.isButton() || !test(interaction.customId)) return;
      emit({ interaction, user: interaction.user, customId: interaction.customId, message: interaction.message });
    });
  },
};
