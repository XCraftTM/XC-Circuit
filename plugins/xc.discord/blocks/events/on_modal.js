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

const idsOf = (text) => String(text || "").split(/\r?\n|,/).map((s) => s.trim()).filter(Boolean);

module.exports = {
  id: "discord.on_modal",
  name: "Modal Submitted",
  description: "Runs when someone sends a form opened with Show Modal. Each text input's answer is an output.",
  author: "XC Team",
  version: "1.0.0",
  category: "interactions",
  icon: "text",
  kind: "event",
  intents: ["Guilds"],
  tags: ["form", "popup", "input", "dialog"],
  fields: {
    customId: { type: "text", pin: false, template: false, required: true, default: "", label: "Custom id", description: "The id given to the modal in Show Modal." },
    match: { type: "select", pin: false, default: "equals", choices: MATCH, description: "How the id is compared." },
    inputIds: { type: "textarea", pin: false, template: false, default: "", label: "Input ids", description: "The ids of the modal's text inputs, one per line. Each becomes an output with the answer." },
  },
  outputs: {
    interaction: { type: "discord.interaction", description: "The form submission. Connect it to Send Message to answer." },
    user: { type: "discord.user", description: "Who sent it." },
    member: { type: "discord.member?", description: "Who sent it, as a server member. Empty in DMs." },
    answers: { type: "object", description: "Every answer, by input id." },
    customId: { type: "text", label: "Custom id", description: "The modal's full id." },
    rest: { type: "text", label: "Rest of id", description: "With Starts with: what comes after the prefix (vote:42 gives 42). With a pattern: its first group." },
  },
  define({ values }) {
    const outputs = {};
    for (const id of idsOf(values.inputIds)) outputs[id] = { type: "text", label: id, description: `What was typed into ${id}.` };
    return { outputs };
  },
  listen(ctx, emit) {
    const test = matcher(ctx.fields.match, ctx.fields.customId);
    const ids = idsOf(ctx.fields.inputIds);
    ctx.discord.client.on("interactionCreate", (interaction) => {
      if (!interaction.isModalSubmit()) return;
      const rest = test(interaction.customId);
      if (rest === null) return;
      const answers = {};
      const all = interaction.fields.fields;
      if (all && typeof all.forEach === "function") all.forEach((c, id) => (answers[id] = c.value ?? ""));
      for (const id of ids) {
        try {
          answers[id] = interaction.fields.getTextInputValue(id);
        } catch {
          answers[id] = answers[id] ?? "";
        }
      }
      emit({ ...answers, interaction, user: interaction.user, member: interaction.member ?? null, answers, customId: interaction.customId, rest });
    });
  },
};
