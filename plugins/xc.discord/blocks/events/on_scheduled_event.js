const KINDS = { created: "An event is created", started: "An event starts", ended: "An event ends", userAdd: "Someone is interested", userRemove: "Someone is no longer interested" };

module.exports = {
  id: "discord.on_scheduled_event",
  name: "Server Event",
  description: "Runs for a server's scheduled events: created, starting, ending, or someone marking interest.",
  author: "XC Team",
  version: "1.0.0",
  category: "events",
  icon: "clock",
  kind: "event",
  intents: ["Guilds", "GuildScheduledEvents"],
  tags: ["scheduled event", "calendar", "stage", "meetup"],
  fields: {
    kind: { type: "select", pin: false, default: "started", label: "When", choices: KINDS, description: "Which event runs it." },
  },
  outputs: {
    event: { type: "object", description: "The scheduled event." },
    name: { type: "text", description: "Its name." },
    user: { type: "discord.user?", description: "Who marked interest (for those)." },
    server: { type: "discord.guild", description: "The server." },
  },
  listen(ctx, emit) {
    const c = ctx.discord.client;
    const out = (e, user = null) => emit({ event: e, name: e.name, user, server: e.guild });
    const kind = ctx.fields.kind;
    if (kind === "created") c.on("guildScheduledEventCreate", (e) => out(e));
    else if (kind === "userAdd") c.on("guildScheduledEventUserAdd", (e, u) => out(e, u));
    else if (kind === "userRemove") c.on("guildScheduledEventUserRemove", (e, u) => out(e, u));
    else
      c.on("guildScheduledEventUpdate", (old, now) => {
        if (!old || old.status === now.status) return;
        if ((kind === "started" && now.isActive()) || (kind === "ended" && now.isCompleted())) out(now);
      });
  },
};
