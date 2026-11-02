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
  id: "discord.on_select",
  name: "Select Menu Used",
  description: "Runs when someone picks from a select menu whose custom id matches.",
  author: "XC Team",
  version: "1.0.0",
  category: "interactions",
  icon: "pointer",
  kind: "event",
  intents: ["Guilds"],
  tags: ["dropdown", "menu", "choose", "component"],
  fields: {
    customId: { type: "text", pin: false, template: false, required: true, default: "", label: "Custom id", description: "The id given to the menu when it was built." },
    match: { type: "select", pin: false, default: "equals", choices: MATCH, description: "How the id is compared." },
  },
  outputs: {
    interaction: { type: "discord.interaction", description: "The menu use. Connect it to Send Message (Update message changes the menu's message)." },
    user: { type: "discord.user", description: "Who picked." },
    member: { type: "discord.member?", description: "Who picked, as a server member. Empty in DMs." },
    value: { type: "text", description: "The first picked value." },
    values: { type: "list<text>", description: "Every picked value (ids for user, role and channel menus)." },
    picked: { type: "list<any>", description: "The picked users, roles or channels themselves (for those menus)." },
    customId: { type: "text", label: "Custom id", description: "The menu's full id." },
    rest: { type: "text", label: "Rest of id", description: "With Starts with: what comes after the prefix (vote:42 gives 42). With a pattern: its first group." },
    channel: { type: "discord.channel", description: "Where it was used." },
    message: { type: "discord.message", description: "The message the menu is on." },
  },
  listen(ctx, emit) {
    const test = matcher(ctx.fields.match, ctx.fields.customId);
    ctx.discord.client.on("interactionCreate", (interaction) => {
      if (!interaction.isAnySelectMenu()) return;
      const rest = test(interaction.customId);
      if (rest === null) return;
      const collections = [interaction.users, interaction.roles, interaction.channels].filter((c) => c && c.size);
      const picked = collections.length ? collections.flatMap((c) => [...c.values()]) : [];
      emit({
        interaction,
        user: interaction.user,
        member: interaction.member ?? null,
        value: interaction.values[0] ?? "",
        values: interaction.values,
        picked,
        customId: interaction.customId,
        rest,
        channel: interaction.channel,
        message: interaction.message,
      });
    });
  },
};
