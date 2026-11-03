module.exports = {
  id: "discord.on_command_permissions",
  kind: "event",
  info: {
    name: "Command Permissions Changed",
    description: "Runs when a server changes who may use one of the bot's commands (Server Settings › Integrations). Bots can't change these themselves; Discord only lets server admins do it.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "commands",
    aliases: ["Command access changed", "Integration permissions"],
    tags: ["permissions", "command", "slash", "integration"],
  },
  look: {
    icon: "key",
  },
  requires: {
    intents: ["Guilds"],
  },
  fields: {},
  outputs: {
    commandId: { type: "text", label: "Command id", description: "The command, or the bot's id when it is about all its commands." },
    command: { type: "text", description: "The command's name, when known." },
    permissions: { type: "list<object>", description: "Each rule: id (role, user or channel), type (role, user, channel) and allowed (yes/no)." },
    server: { type: "discord.guild?", description: "The server." },
  },
  listen(ctx, emit) {
    const c = ctx.discord.client;
    const TYPES = { 1: "role", 2: "user", 3: "channel" };
    c.on("applicationCommandPermissionsUpdate", (data) => {
      const server = c.guilds.cache.get(data.guildId) ?? null;
      const command = c.application?.commands.cache.get(data.id)?.name ?? server?.commands.cache.get(data.id)?.name ?? "";
      emit({ commandId: data.id, command, permissions: (data.permissions || []).map((p) => ({ id: p.id, type: TYPES[p.type] ?? String(p.type), allowed: Boolean(p.permission) })), server });
    });
  },
};
