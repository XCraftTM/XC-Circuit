const STYLES = {
  bold: "Bold",
  italic: "Italic",
  underline: "Underline",
  strike: "Strikethrough",
  spoiler: "Spoiler",
  code: "Code",
  codeblock: "Code block",
  quote: "Quote",
  h1: "Big heading",
  h2: "Heading",
  h3: "Small heading",
  subtext: "Small gray text",
  list: "Bullet list (one item per line)",
  link: "Link with text",
  escape: "Show as typed (no formatting)",
};

module.exports = {
  id: "discord.discord_text",
  name: "Discord Text",
  description: "Formats text with Discord markdown: bold, code blocks, spoilers, headings, quotes, links …",
  author: "XC Team",
  version: "1.0.0",
  category: "format",
  icon: "text",
  kind: "value",
  tags: ["markdown", "bold", "code", "spoiler", "format"],
  fields: {
    text: { type: "textarea", pin: true, default: "", description: "The text." },
    style: { type: "select", pin: false, default: "bold", choices: STYLES, description: "How to format it." },
    language: { type: "text", default: "", show: { style: ["codeblock"] }, description: "For colors, such as js, py or json." },
    url: { type: "text", default: "https://", show: { style: ["link"] }, description: "Where the link goes." },
  },
  outputs: {
    text: { type: "text", description: "The formatted text." },
  },
  run({ fields: f }) {
    const t = String(f.text ?? "");
    const lines = (prefix) => t.split("\n").map((l) => prefix + l).join("\n");
    const out = {
      bold: () => `**${t}**`,
      italic: () => `*${t}*`,
      underline: () => `__${t}__`,
      strike: () => `~~${t}~~`,
      spoiler: () => `||${t}||`,
      code: () => `\`${t.replace(/`/g, "ˋ")}\``,
      codeblock: () => "```" + (f.language || "") + "\n" + t + "\n```",
      quote: () => lines("> "),
      h1: () => `# ${t}`,
      h2: () => `## ${t}`,
      h3: () => `### ${t}`,
      subtext: () => lines("-# "),
      list: () => lines("- "),
      link: () => `[${t}](${f.url})`,
      escape: () => t.replace(/([\\*_~`|>#\-\[\]()])/g, "\\$1"),
    }[f.style];
    return { text: out ? out() : t };
  },
};
