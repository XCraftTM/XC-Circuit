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
  kind: "event",
  info: {
    name: "Modal Submitted",
    description: "Runs when someone sends a form opened with Show Modal. Each text input's answer is an output.",
    author: "XC Team",
    version: "1.2.0",
  },
  search: {
    category: "components",
    aliases: ["Form submitted"],
    tags: ["form", "popup", "input", "dialog"],
  },
  look: {
    icon: "text",
  },
  requires: {
    intents: ["Guilds"],
  },
  fields: {
    customId: { type: "text", pin: false, template: false, required: true, default: "", suggest: "form", label: "Custom id", description: "The id given to the form in Show Modal: pick one, or type it." },
    match: { type: "select", pin: false, default: "equals", choices: MATCH, description: "How the id is compared." },
    inputIds: { type: "textarea", pin: false, template: false, default: "", label: "Input ids", description: "The ids of the modal's text inputs, one per line. Each becomes an output with the answer." },
  },
  outputs: {
    interaction: { type: "discord.interaction", description: "The form submission. Connect it to Send Message to answer." },
    user: { type: "discord.user", description: "Who sent it." },
    member: { type: "discord.member?", description: "Who sent it, as a server member. Empty in DMs." },
    answers: { type: "object", description: "Every answer, by input id: the text, the chosen values of a menu, or the files of an upload." },
    files: { type: "list<object>", description: "Every uploaded file (name, url, size, contentType)." },
    picked: { type: "object", description: "For user, role, channel and mentionable menus: what was picked (the users, roles or channels), by input id." },
    customId: { type: "text", label: "Custom id", description: "The modal's full id." },
    rest: { type: "text", label: "Rest of id", description: "With Starts with: what comes after the prefix (vote:42 gives 42). With a pattern: its first group." },
    channel: { type: "discord.channel", description: "Where the form was sent from." },
    server: { type: "discord.guild?", description: "The server it was sent in. Empty in DMs." },
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
      const texts = {};
      const files = [];
      const picked = {};
      const all = interaction.fields.fields;
      if (all && typeof all.forEach === "function") {
        all.forEach((c, id) => {
          if (c.attachments) {
            // A file upload: its files, and their links as text.
            const list = [...c.attachments.values()];
            files.push(...list);
            answers[id] = list;
            texts[id] = list.map((a) => a.url).join("\n");
          } else if (Array.isArray(c.values)) {
            // A menu: the chosen values, and what they are for user, role and channel menus.
            answers[id] = [...c.values];
            texts[id] = c.values.join(", ");
            const objects = [c.members, c.users, c.roles, c.channels].find((x) => x && x.size);
            if (objects) picked[id] = [...objects.values()];
          } else {
            answers[id] = c.value ?? "";
            texts[id] = answers[id];
          }
        });
      }
      for (const id of ids) {
        if (id in texts) continue;
        try {
          texts[id] = answers[id] = interaction.fields.getTextInputValue(id);
        } catch {
          texts[id] = answers[id] = answers[id] ?? "";
        }
      }
      emit({ ...texts, files, picked, interaction, user: interaction.user, member: interaction.member ?? null, answers, customId: interaction.customId, rest, channel: interaction.channel, server: interaction.guild ?? null });
    });
  },
};
