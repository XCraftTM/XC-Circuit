module.exports = {
  id: "discord.create_poll",
  kind: "action",
  info: {
    name: "Create Poll",
    description: "Posts a Discord poll. Read the results with Poll Results.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "reactions",
    aliases: ["Vote", "Survey", "Umfrage erstellen"],
    tags: ["vote", "survey"],
  },
  look: {
    icon: "scale",
  },
  fields: {
    channel: { type: "discord.channel", pin: "only", required: true, description: "Where to post it." },
    question: { type: "text", required: true, default: "What do you think?", description: "The question (up to 300 characters)." },
    answers: { type: "textarea", required: true, default: "Yes\nNo", description: "One answer per line (up to 10). Start a line with an emoji and a space to give it one." },
    hours: { type: "number", default: 24, min: 1, max: 768, step: 1, description: "How long it stays open." },
    multiple: { type: "boolean", default: false, label: "Several answers", description: "People may pick more than one." },
  },
  outputs: {
    message: { type: "discord.message", description: "The poll's message." },
  },
  async run({ fields: f }) {
    const answers = String(f.answers || "").split(/\r?\n/).map((l) => l.trim()).filter(Boolean).slice(0, 10).map((line) => {
      const m = /^(\p{Extended_Pictographic}\uFE0F?|<a?:\w+:\d+>)\s+(.+)$/u.exec(line);
      return m ? { text: m[2].slice(0, 55), emoji: m[1] } : { text: line.slice(0, 55) };
    });
    if (answers.length < 1) throw new Error("A poll needs at least one answer");
    const message = await f.channel.send({
      poll: { question: { text: String(f.question).slice(0, 300) }, answers, duration: Math.max(1, Math.min(768, Math.floor(f.hours || 24))), allowMultiselect: Boolean(f.multiple) },
    });
    return { message };
  },
};
