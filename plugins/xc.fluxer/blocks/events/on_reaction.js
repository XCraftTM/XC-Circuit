const WHEN = { add: "Added", remove: "Removed" };

module.exports = {
  id: "fluxer.on_reaction",
  kind: "event",
  info: {
    name: "Reaction Added or Removed",
    description: "Runs when someone adds a reaction to a message, or takes theirs away. For reaction roles and polls.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "reactions",
    aliases: ["On reaction", "Reaction role", "Emoji added"],
  },
  look: {
    icon: "check",
  },
  fields: {
    when: { type: "select", pin: false, default: "add", choices: WHEN, label: "When", description: "Reactions added or removed." },
    emoji: { type: "text", pin: false, default: "", template: false, description: "Only this emoji: 👍, or a custom emoji's name. Empty = every emoji." },
    messageId: { type: "text", pin: false, default: "", template: false, label: "Message id", description: "Only reactions on this message. Empty = every message." },
    ignoreBots: { type: "boolean", pin: false, default: true, label: "Ignore bots", description: "Skip reactions by bots, including this one." },
  },
  outputs: {
    user: { type: "fluxer.user", description: "Who reacted." },
    member: { type: "fluxer.member?", description: "Who reacted, as a server member. Empty in direct messages." },
    message: { type: "fluxer.message?", description: "The message, when it could be loaded." },
    server: { type: "fluxer.server?", description: "The server it happened in. Empty in direct messages." },
    emoji: { type: "text", description: "The emoji: 👍, or <:name:id> for a custom one." },
    messageId: { type: "text", label: "Message id", description: "The message's id." },
    channelId: { type: "text", label: "Channel id", description: "Where the message is." },
    serverId: { type: "text", label: "Server id", description: "The server it happened in. Empty in direct messages." },
  },
  listen(ctx, emit) {
    // The server itself, found in the bot's servers by its id.
    const send = emit;
    emit = (o) => send({ ...o, server: (o.serverId && ctx.fluxer.client.guilds?.get?.(String(o.serverId))) || null });
    const event = ctx.fields.when === "remove" ? "messageReactionRemove" : "messageReactionAdd";
    const want = String(ctx.fields.emoji || "").trim();
    ctx.fluxer.client.on(event, async (p) => {
      const e = p.emoji || {};
      if (want && want !== e.name && want !== `<:${e.name}:${e.id}>` && want !== e.id) return;
      if (ctx.fields.messageId && String(ctx.fields.messageId).trim() !== p.messageId) return;
      const user = p.user ?? (await ctx.fluxer.client.users.resolve(p.userId).catch(() => null));
      if (ctx.fields.ignoreBots && user?.bot) return;
      const message = p.message ?? (await p.reaction?.fetchMessage?.().catch(() => null)) ?? null;
      emit({ emoji: e.id ? `<${e.animated ? "a" : ""}:${e.name}:${e.id}>` : e.name, user, member: p.member ?? null, message, messageId: p.messageId, channelId: p.channelId, serverId: String(p.guildId ?? p.guild_id ?? "") });
    });
  },
};
