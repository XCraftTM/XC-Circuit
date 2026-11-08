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

/** What each kind of form input gives (the forms come from the Modal Builder). */
const KINDS = {
  text: { type: "text", says: "What was typed" },
  options: { type: "list<text>", says: "The options picked (their values)" },
  users: { type: "list<discord.member>", says: "The members picked (users, in DMs)" },
  roles: { type: "list<discord.role>", says: "The roles picked" },
  mentionables: { type: "list<any>", says: "The members and roles picked" },
  channels: { type: "list<discord.channel>", says: "The channels picked" },
  files: { type: "list<object>", says: "The files uploaded (name, url, size, contentType)" },
};

/** The outputs every Modal Submitted has: a form's input with such an id doesn't replace one. */
const FIXED = ["interaction", "user", "member", "answers", "files", "picked", "customId", "rest", "channel", "server"];

module.exports = {
  id: "discord.on_modal",
  kind: "event",
  info: {
    name: "Modal Submitted",
    description: "Runs when someone sends a form opened with Show Modal. Each of the form's inputs is an output, of its kind: a text, or the members, roles, channels or files picked.",
    author: "XC Team",
    version: "1.3.0",
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
    inputIds: { type: "textarea", pin: false, template: false, default: "", label: "Input ids", description: "Only for forms not made in the Modal Builder (those give their inputs by themselves): the ids of its inputs, one per line. Each becomes an output with the answer as text." },
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
  define({ values, project }) {
    const outputs = {};
    // Ids typed in Input ids give text (as they always did).
    for (const id of idsOf(values.inputIds)) if (!FIXED.includes(id)) outputs[id] = { type: "text", label: id, description: `What was typed into ${id}.` };
    // The inputs of the forms this event takes, as designed in the Modal Builder.
    let test = () => null;
    try {
      test = matcher(values.match, values.customId);
    } catch {}
    for (const form of (project && project.forms) || []) {
      if (!values.customId || test(String(form.customId)) === null) continue;
      for (const input of form.inputs || []) {
        const kind = KINDS[input.kind];
        if (!kind || FIXED.includes(input.id) || outputs[input.id]) continue;
        outputs[input.id] = { type: kind.type, label: input.label || input.id, description: `${kind.says} in ${input.label || input.id} (${input.id}).` };
      }
    }
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
      const typed = {};
      const files = [];
      const picked = {};
      const all = interaction.fields.fields;
      if (all && typeof all.forEach === "function") {
        all.forEach((c, id) => {
          if (c.attachments) {
            // A file upload: its files, and their links as text.
            const list = [...c.attachments.values()];
            files.push(...list);
            answers[id] = typed[id] = list;
            texts[id] = list.map((a) => a.url).join("\n");
          } else if (Array.isArray(c.values)) {
            // A menu: the chosen values, and what they are for user, role and channel menus.
            answers[id] = [...c.values];
            texts[id] = c.values.join(", ");
            // Members (users in DMs), roles and channels; a text menu's values.
            const all = (x) => (x && x.size ? [...x.values()] : []);
            const people = all(c.members).length ? all(c.members) : all(c.users);
            const objects = [...people, ...all(c.roles), ...all(c.channels)];
            if (objects.length) picked[id] = objects;
            typed[id] = objects.length ? objects : [...c.values];
          } else {
            answers[id] = c.value ?? "";
            texts[id] = typed[id] = answers[id];
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
      // Each input by its id, of its kind; ids in Input ids as text.
      const asText = Object.fromEntries(ids.map((id) => [id, texts[id]]));
      emit({ ...typed, ...asText, files, picked, interaction, user: interaction.user, member: interaction.member ?? null, answers, customId: interaction.customId, rest, channel: interaction.channel, server: interaction.guild ?? null });
    });
  },
};
