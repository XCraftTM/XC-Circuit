const WHEN = { ban: "Banned", unban: "Unbanned" };

module.exports = {
  id: "fluxer.on_ban",
  kind: "event",
  info: {
    name: "Member Banned or Unbanned",
    description: "Runs when someone is banned from a server, or their ban is lifted. For mod logs.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "moderation",
    aliases: ["On ban", "On unban", "Ban log"],
  },
  look: {
    icon: "shield",
  },
  fields: {
    when: { type: "select", pin: false, default: "ban", choices: WHEN, label: "When", description: "Bans or lifted bans." },
  },
  outputs: {
    user: { type: "fluxer.user", description: "Who was banned or unbanned." },
    server: { type: "fluxer.server?", description: "The server." },
    reason: { type: "text", description: "The reason given, if any." },
    until: { type: "date?", description: "When a ban for a while ends. Empty for bans without an end." },
  },
  listen(ctx, emit) {
    const event = ctx.fields.when === "unban" ? "guildBanRemove" : "guildBanAdd";
    ctx.fluxer.client.on(event, (ban) => {
      const server = ctx.fluxer.client.guilds?.get?.(ban.guildId) ?? null;
      emit({ user: ban.user, server, reason: ban.reason ?? "", until: ban.expiresAt ?? null });
    });
  },
};
