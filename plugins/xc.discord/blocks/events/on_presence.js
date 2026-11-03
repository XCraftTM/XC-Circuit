module.exports = {
  id: "discord.on_presence",
  kind: "event",
  info: {
    name: "Presence Changed",
    description: "Runs when someone's status or activity changes (online, playing a game …). Needs the Presence intent turned on in the developer portal.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_members",
    aliases: ["Status changed", "Online"],
    tags: ["status", "online", "playing", "streaming"],
  },
  look: {
    icon: "user",
  },
  requires: {
    intents: ["Guilds", "GuildMembers", "GuildPresences"],
  },
  fields: {
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Skip bots." },
  },
  outputs: {
    member: { type: "discord.member?", description: "Who changed." },
    before: { type: "text", description: "Their status before: online, idle, dnd or offline." },
    status: { type: "text", description: "Their status now." },
    activity: { type: "text", description: "What they're doing now, such as a game's name." },
    streaming: { type: "boolean", description: "Whether they are streaming." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("presenceUpdate", (old, now) => {
      if (ctx.fields.ignoreBots && now.user?.bot) return;
      const main = now.activities.find((a) => a.type !== 4) ?? now.activities[0];
      emit({ member: now.member ?? null, before: old?.status ?? "offline", status: now.status, activity: main ? main.state ?? main.name : "", streaming: now.activities.some((a) => a.type === 1) });
    });
  },
};
