module.exports = {
  id: "discord.build_embed",
  name: "Build Embed",
  description: "Creates a rich embed to attach to a message.",
  author: "XC Team",
  version: "1.0.0",
  category: "messages",
  icon: "embed",
  kind: "action",
  fields: {
    title: { type: "text", template: true, default: "", description: "Bold heading at the top." },
    description: { type: "textarea", template: true, default: "", description: "The main text." },
    color: { type: "color", pin: false, default: "#00D8F0", description: "The colored bar on the left." },
    footer: { type: "text", template: true, default: "", advanced: true, description: "Small text at the bottom." },
    image: { type: "text", default: "", advanced: true, label: "Image URL", description: "A large image under the text." },
    thumbnail: { type: "text", default: "", advanced: true, label: "Thumbnail URL", description: "A small image in the top right." },
    timestamp: { type: "boolean", pin: false, default: false, advanced: true, description: "Show the current time in the footer." },
  },
  outputs: {
    embed: { type: "discord.embed", description: "The finished embed. Connect it to Send Message." },
  },
  run({ fields }) {
    const { EmbedBuilder } = require("discord.js");
    const embed = new EmbedBuilder().setColor(fields.color || null);
    if (fields.title) embed.setTitle(fields.title);
    if (fields.description) embed.setDescription(fields.description);
    if (fields.footer) embed.setFooter({ text: fields.footer });
    if (fields.image) embed.setImage(fields.image);
    if (fields.thumbnail) embed.setThumbnail(fields.thumbnail);
    if (fields.timestamp) embed.setTimestamp();
    return { embed };
  },
};
