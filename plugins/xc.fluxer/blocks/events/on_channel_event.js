const WHEN = { create: "Created", update: "Changed", delete: "Deleted" };

module.exports = {
  id: "fluxer.on_channel_event",
  kind: "event",
  info: {
    name: "Channel Created, Changed or Deleted",
    description: "Runs when a channel is made, changed (name, topic, permissions …) or deleted in a server.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["On channel", "Channel created", "Channel deleted", "Channel updated"],
  },
  look: {
    icon: "hash",
  },
  fields: {
    when: { type: "select", pin: false, default: "create", choices: WHEN, label: "When", description: "Which change." },
  },
  outputs: {
    channel: { type: "fluxer.channel", description: "The channel (as it is now, or was before deleting)." },
    before: { type: "fluxer.channel?", description: "For changes: the channel as it was." },
    server: { type: "fluxer.server?", description: "The server." },
    name: { type: "text", description: "Its name." },
  },
  listen(ctx, emit) {
    const client = ctx.fluxer.client;
    const out = (channel, before = null) => emit({ channel, before, name: channel?.name ?? "", server: channel?.guild ?? client.guilds?.get?.(channel?.guildId) ?? null });
    if (ctx.fields.when === "update") client.on("channelUpdate", (before, channel) => out(channel, before));
    else client.on(ctx.fields.when === "delete" ? "channelDelete" : "channelCreate", (channel) => out(channel));
  },
};
