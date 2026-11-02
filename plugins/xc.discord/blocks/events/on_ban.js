const KINDS = { ban: "Banned", unban: "Unbanned" };

module.exports = {
  id: "discord.on_ban",
  name: "Member Banned",
  description: "Runs when someone is banned from (or unbanned in) a server.",
  author: "XC Team",
  version: "1.0.0",
  category: "events",
  icon: "shield",
  kind: "event",
  intents: ["Guilds", "GuildModeration"],
  tags: ["log", "unban", "moderation"],
  fields: {
    kind: { type: "select", pin: false, default: "ban", label: "When", choices: KINDS, description: "Bans or unbans." },
  },
  outputs: {
    user: { type: "discord.user", description: "Who was banned or unbanned." },
    server: { type: "discord.guild", description: "The server." },
    reason: { type: "text", description: "The reason, when one was given." },
  },
  listen(ctx, emit) {
    const event = ctx.fields.kind === "unban" ? "guildBanRemove" : "guildBanAdd";
    ctx.discord.client.on(event, (ban) => emit({ user: ban.user, server: ban.guild, reason: ban.reason ?? "" }));
  },
};
