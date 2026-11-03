module.exports = {
  id: "discord.poll_results",
  kind: "action",
  info: {
    name: "Poll Results",
    description: "Reads a poll's votes, and can end it now.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "reactions",
    aliases: ["Votes", "Umfrageergebnisse"],
    tags: ["votes", "end poll", "winner"],
  },
  look: {
    icon: "scale",
  },
  fields: {
    message: { type: "discord.message", pin: "only", required: true, description: "The poll's message." },
    end: { type: "boolean", default: false, label: "End it now", description: "Close the poll before reading." },
  },
  outputs: {
    results: { type: "list<object>", description: "Each answer with text and votes, most votes first." },
    winner: { type: "text", description: "The answer with the most votes." },
    total: { type: "number", description: "All votes together." },
  },
  async run({ fields: f }) {
    let message = f.message.partial ? await f.message.fetch() : f.message;
    if (!message.poll) throw new Error("That message has no poll");
    if (f.end && !message.poll.resultsFinalized) message = await message.poll.end();
    const results = [...message.poll.answers.values()].map((a) => ({ text: a.text, votes: a.voteCount ?? 0 })).sort((a, b) => b.votes - a.votes);
    return { results, winner: results[0]?.text ?? "", total: results.reduce((n, r) => n + r.votes, 0) };
  },
};
