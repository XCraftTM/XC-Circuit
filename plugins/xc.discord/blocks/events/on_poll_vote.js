const KINDS = { add: "A vote is added", remove: "A vote is taken back" };

module.exports = {
  id: "discord.on_poll_vote",
  kind: "event",
  info: {
    name: "Poll Vote",
    description: "Runs when someone votes in a poll (or takes a vote back).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_messages",
    aliases: ["Vote", "Abstimmung"],
    tags: ["poll", "vote"],
  },
  look: {
    icon: "scale",
  },
  requires: {
    intents: ["Guilds", "GuildMessagePolls"],
  },
  fields: {
    kind: { type: "select", pin: false, default: "add", label: "When", choices: KINDS, description: "Votes added or removed." },
  },
  outputs: {
    answer: { type: "text", description: "The answer's text." },
    userId: { type: "text", label: "User id", description: "Who voted." },
    message: { type: "discord.message", description: "The poll's message." },
  },
  listen(ctx, emit) {
    const event = ctx.fields.kind === "remove" ? "messagePollVoteRemove" : "messagePollVoteAdd";
    ctx.discord.client.on(event, (answer, userId) => emit({ answer: answer.text ?? "", userId, message: answer.poll?.message }));
  },
};
