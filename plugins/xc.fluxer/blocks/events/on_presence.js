module.exports = {
  id: "fluxer.on_presence",
  kind: "event",
  info: {
    name: "Status Changed",
    description: "Runs when someone goes online, idle, do not disturb or offline, or changes what they're doing.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "members",
    aliases: ["On presence", "Went online", "Went offline", "Activity changed"],
  },
  look: {
    icon: "user",
  },
  fields: {},
  outputs: {
    userId: { type: "text", label: "User id", description: "Who." },
    status: { type: "text", description: "online, idle, dnd or offline." },
    activity: { type: "text", description: "What they're doing, when shown (a game, a custom status …)." },
    serverId: { type: "text", label: "Server id", description: "The server it was seen in." },
  },
  listen(ctx, emit) {
    ctx.fluxer.client.on("presenceUpdate", (p) => {
      const first = (p?.activities || [])[0];
      emit({
        userId: String(p?.user?.id ?? p?.userId ?? p?.user_id ?? ""),
        status: String(p?.status ?? ""),
        activity: String(first?.state ?? first?.name ?? ""),
        serverId: String(p?.guildId ?? p?.guild_id ?? ""),
      });
    });
  },
};
