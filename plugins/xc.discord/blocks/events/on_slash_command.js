// Option types of the Slash Command Builder and the pin type each one gives.
const PIN_TYPES = {
  string: "text",
  integer: "number",
  number: "number",
  boolean: "boolean",
  user: "discord.user",
  channel: "discord.channel",
  role: "discord.role",
  mentionable: "any",
  attachment: "object",
};

const commandsOf = (project) => (project && project.data && project.data.slashCommands) || [];

// An option named like a built-in output (such as "user") gets "Option" added.
const BUILT_IN = ["interaction", "user", "member", "channel", "server"];
const outputKey = (name) => (BUILT_IN.includes(name) ? `${name}Option` : name);

module.exports = {
  id: "discord.on_slash_command",
  kind: "event",
  info: {
    name: "Slash Command",
    description: "Runs when someone uses one of your slash commands. Build them in Bot › Slash Commands.",
    author: "XC Team",
    version: "2.1.0",
  },
  search: {
    category: "events",
  },
  look: {
    icon: "slash",
  },
  requires: {
    intents: ["Guilds"],
  },
  fields: {
    command: {
      type: "slash-command-ref",
      pin: false,
      required: true,
      default: "",
      description: "Which command runs this block.",
    },
  },
  outputs: {
    interaction: { type: "discord.interaction", description: "The command use. Connect it to Send Message." },
    user: { type: "discord.user", description: "Who used the command." },
    member: { type: "discord.member?", description: "Who used it, as a server member. Empty in DMs." },
    channel: { type: "discord.channel", description: "Where it was used." },
    server: { type: "discord.guild?", description: "The server it was used in. Empty in DMs." },
  },

  define({ values, project }) {
    const commands = commandsOf(project);
    // Subcommands are listed by their full name (ticket open); a command or group
    // that only holds subcommands can't be used on its own, so it isn't listed.
    const isHeader = (c) => commands.some((o) => o.name.startsWith(`${c.name} `));
    const choices = Object.fromEntries(commands.filter((c) => !isHeader(c)).map((c) => [c.name, `/${c.name}`]));
    if (values.command && !(values.command in choices)) choices[values.command] = `/${values.command} (not in the builder)`;
    const command = commands.find((c) => c.name === values.command);
    const outputs = {};
    for (const o of (command && command.options) || []) {
      const optional = o.required ? "" : "?";
      outputs[outputKey(o.name)] = {
        type: (PIN_TYPES[o.type] || "any") + optional,
        label: outputKey(o.name) === o.name ? o.name : `${o.name} (option)`,
        description: o.description || `The ${o.name} option.`,
      };
    }
    return { set: { command: { choices } }, outputs };
  },

  migrate(values, from) {
    // 1.x kept a description on the block; it now lives in the Slash Command Builder.
    if (from === "1") return { command: values.command || "" };
    return values;
  },

  listen(ctx, emit) {
    const name = String(ctx.fields.command || "").toLowerCase().trim().replace(/\s+/g, " ");
    const command = commandsOf(ctx.project).find((c) => c.name === name);
    ctx.discord.client.on("interactionCreate", (interaction) => {
      if (!interaction.isChatInputCommand()) return;
      const used = [interaction.commandName, interaction.options.getSubcommandGroup(false), interaction.options.getSubcommand(false)].filter(Boolean).join(" ");
      if (used !== name) return;
      const options = {};
      for (const o of (command && command.options) || []) {
        const read = {
          user: () => interaction.options.getUser(o.name),
          channel: () => interaction.options.getChannel(o.name),
          role: () => interaction.options.getRole(o.name),
          mentionable: () => interaction.options.getMentionable(o.name),
          attachment: () => interaction.options.getAttachment(o.name),
        }[o.type];
        options[outputKey(o.name)] = read ? read() : interaction.options.get(o.name)?.value ?? null;
      }
      emit({
        ...options,
        interaction,
        user: interaction.user,
        member: interaction.member ?? null,
        channel: interaction.channel,
        server: interaction.guild ?? null,
      });
    });
  },
};
