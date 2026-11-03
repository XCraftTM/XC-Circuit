const STYLES = { primary: "Blurple", secondary: "Grey", success: "Green", danger: "Red", link: "Link (opens a URL)" };

module.exports = {
  id: "discord.button",
  kind: "value",
  info: {
    name: "Button",
    description: "A button to attach to a message. Presses arrive at the Button Pressed event with the same custom id.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "components",
    aliases: ["Link button"],
    tags: ["component", "click"],
  },
  look: {
    icon: "pointer",
  },
  fields: {
    label: { type: "text", template: true, default: "Click me", description: "The text on the button." },
    style: { type: "select", pin: false, default: "primary", choices: STYLES, description: "The color of the button, or Link to open a web page." },
    customId: { type: "text", default: "my-button", label: "Custom id", description: "Identifies the button in Button Pressed.", show: { style: ["primary", "secondary", "success", "danger"] } },
    url: { type: "text", default: "https://", label: "URL", show: { style: ["link"] }, description: "The web page a Link button opens." },
    emoji: { type: "text", default: "", advanced: true, description: "An emoji such as 👍 or <:name:id>." },
    disabled: { type: "boolean", default: false, advanced: true, description: "Shown grayed out, so nobody can press it." },
  },
  outputs: {
    button: { type: "discord.component", description: "Connect to Send Message › Components." },
  },
  run({ fields }) {
    const { ButtonBuilder, ButtonStyle } = require("discord.js");
    const style = { primary: ButtonStyle.Primary, secondary: ButtonStyle.Secondary, success: ButtonStyle.Success, danger: ButtonStyle.Danger, link: ButtonStyle.Link }[fields.style];
    const b = new ButtonBuilder().setStyle(style).setLabel(String(fields.label || " ").slice(0, 80)).setDisabled(Boolean(fields.disabled));
    if (fields.style === "link") b.setURL(fields.url);
    else b.setCustomId(String(fields.customId || "button").slice(0, 100));
    if (fields.emoji) b.setEmoji(fields.emoji);
    return { button: b };
  },
};
