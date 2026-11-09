const KINDS = { settings: "Server settings changed", integrations: "Integrations changed", webhooks: "A channel's webhooks changed" };

// What can change on a server, compared before and after.
const KEYS = ["name", "icon", "banner", "description", "ownerId", "verificationLevel", "explicitContentFilter", "defaultMessageNotifications", "afkChannelId", "afkTimeout", "systemChannelId", "rulesChannelId", "publicUpdatesChannelId", "preferredLocale", "premiumTier", "vanityURLCode", "mfaLevel", "nsfwLevel"];

module.exports = {
  id: "discord.on_server_update",
  kind: "event",
  info: {
    name: "Server Changed",
    description: "Runs when a server's settings change (name, icon, owner, verification …), when its integrations change, or when a channel's webhooks change.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Server updated", "Guild update", "Webhooks changed", "Integrations changed"],
    tags: ["server", "guild", "settings", "webhook", "integration", "log"],
  },
  look: {
    icon: "layers",
  },
  requires: {
    intents: ["Guilds", "GuildIntegrations", "GuildWebhooks"],
  },
  fields: {
    kind: { type: "select", pin: false, default: "settings", label: "When", choices: KINDS, description: "Which event runs it." },
  },
  outputs: {
    server: { type: "discord.guild", description: "The server (as it is now)." },
    channel: { type: "discord.channel?", description: "For webhooks: the channel whose webhooks changed." },
    beforeName: { type: "text", label: "Name before", description: "For settings: its name before." },
    changed: { type: "list<text>", description: "For settings: what changed, such as name or icon." },
    changes: { type: "list<object>", description: "For settings: each change with its old and new value." },
  },
  listen(ctx, emit) {
    const c = ctx.discord.client;
    const kind = ctx.fields.kind;
    if (kind === "settings") {
      c.on("guildUpdate", (old, now) => {
        const changes = KEYS.filter((k) => old[k] !== now[k]).map((k) => ({ key: k, old: old[k] ?? null, new: now[k] ?? null }));
        if (changes.length) emit({ server: now, beforeName: old.name ?? "", changed: changes.map((x) => x.key), changes, channel: null });
      });
    } else if (kind === "integrations") {
      c.on("guildIntegrationsUpdate", (server) => emit({ server, beforeName: server.name, changed: [], changes: [], channel: null }));
    } else {
      c.on("webhooksUpdate", (channel) => emit({ server: channel.guild, beforeName: channel.guild.name, changed: [], changes: [], channel }));
    }
  },
};
