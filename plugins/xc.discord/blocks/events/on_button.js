const MATCH = { equals: "Equals", starts: "Starts with", pattern: "Matches pattern" };

/** Tests a custom id. Gives the rest of it (after a prefix, or a pattern's first group), or null. */
function matcher(match, wanted) {
  wanted = String(wanted || "");
  if (match === "starts") return (id) => (id.startsWith(wanted) ? id.slice(wanted.length) : null);
  if (match === "pattern") {
    const re = new RegExp(wanted);
    return (id) => {
      const m = re.exec(id);
      return m ? (m[1] ?? m[0]) : null;
    };
  }
  return (id) => (id === wanted ? "" : null);
}

module.exports = {
  id: "discord.on_button",
  kind: "event",
  info: {
    name: "Button Pressed",
    description: "Runs when someone presses a button whose custom id matches.",
    author: "XC Team",
    version: "1.3.0",
  },
  search: {
    category: "components",
    aliases: ["Button clicked", "Click"],
  },
  look: {
    icon: "pointer",
  },
  requires: {
    intents: ["Guilds"],
  },
  fields: {
    customId: { type: "text", pin: false, template: false, required: true, default: "", suggest: "button", label: "Custom id", description: "The id given to the button when it was built: pick one your layouts and Button blocks use, or type it." },
    match: { type: "select", pin: false, default: "equals", choices: MATCH, description: "How the id is compared. Starts with lets one block handle buttons like vote:1, vote:2 …" },
  },
  outputs: {
    interaction: { type: "discord.interaction", description: "The button press. Connect it to Send Message." },
    user: { type: "discord.user", description: "Who pressed it." },
    member: { type: "discord.member?", description: "Who pressed it, as a server member. Empty in DMs." },
    customId: { type: "text", label: "Custom id", description: "The full id of the pressed button." },
    rest: { type: "text", label: "Rest of id", description: "With Starts with: what comes after the prefix (vote:42 gives 42). With a pattern: its first group." },
    channel: { type: "discord.channel", description: "Where it was used." },
    message: { type: "discord.message", description: "The message the button is on." },
    server: { type: "discord.guild?", description: "The server it was pressed in. Empty in DMs." },
  },
  listen(ctx, emit) {
    const test = matcher(ctx.fields.match, ctx.fields.customId);
    ctx.discord.client.on("interactionCreate", (interaction) => {
      if (!interaction.isButton()) return;
      const rest = test(interaction.customId);
      if (rest === null) return;
      emit({ interaction, user: interaction.user, member: interaction.member ?? null, customId: interaction.customId, rest, channel: interaction.channel, message: interaction.message, server: interaction.guild ?? null });
    });
  },
};
