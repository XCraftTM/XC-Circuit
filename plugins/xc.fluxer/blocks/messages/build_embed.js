module.exports = {
  id: "fluxer.build_embed",
  kind: "action",
  info: {
    name: "Build Embed",
    description: "Creates a rich embed. Design it in the Embed Builder; ${…} in its texts holds JavaScript.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "design",
    aliases: ["Embed", "Rich message"],
  },
  look: {
    icon: "embed",
    preview: "embed",
  },
  fields: {
    embed: {
      type: "embed",
      pin: false,
      template: true,
      label: "Embed",
      default: { color: "#00D8F0", title: "", description: "", fields: [] },
      description: "The embed design. Open the Embed Builder to edit it.",
    },
  },
  outputs: {
    embed: { type: "fluxer.embed", description: "The finished embed. Connect it to Send Message." },
  },
  run({ fields, ctx }) {
    const embed = ctx.fluxer.embed(fields.embed, (s) => ctx.fill(s, fields));
    if (!embed.title && !embed.description && !embed.fields && !embed.image && !embed.thumbnail) {
      throw new Error("The embed is empty: give it a title, text, a field or a picture");
    }
    return { embed };
  },
};
