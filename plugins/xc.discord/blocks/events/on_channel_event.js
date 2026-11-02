const KINDS = { created: "Created", deleted: "Deleted", updated: "Changed" };

module.exports = {
  id: "discord.on_channel_event",
  name: "Channel Created / Deleted",
  description: "Runs when a channel is created, deleted or changed. For server logs.",
  author: "XC Team",
  version: "1.0.0",
  category: "events",
  icon: "hash",
  kind: "event",
  intents: ["Guilds"],
  tags: ["log", "channel update", "server log"],
  fields: {
    kind: { type: "select", pin: false, default: "created", label: "When", choices: KINDS, description: "Which event runs it." },
  },
  outputs: {
    channel: { type: "discord.channel", description: "The channel (as it is now, or was before deleting)." },
    beforeName: { type: "text", label: "Name before", description: "Its name before a change." },
    server: { type: "discord.guild?", description: "The server." },
  },
  listen(ctx, emit) {
    const c = ctx.discord.client;
    const kind = ctx.fields.kind;
    if (kind === "updated") c.on("channelUpdate", (old, now) => emit({ channel: now, beforeName: old.name ?? "", server: now.guild ?? null }));
    else c.on(kind === "deleted" ? "channelDelete" : "channelCreate", (ch) => emit({ channel: ch, beforeName: ch.name ?? "", server: ch.guild ?? null }));
  },
};
