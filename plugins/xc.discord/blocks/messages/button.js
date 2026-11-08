const STYLES = { primary: "Blurple", secondary: "Grey", success: "Green", danger: "Red", link: "Link (opens a URL)", premium: "Premium (buy an item)" };

module.exports = {
  id: "discord.button",
  kind: "value",
  info: {
    name: "Button",
    description: "A button to attach to a message. Presses arrive at the Button Pressed event with the same custom id.",
    author: "XC Team",
    version: "1.2.0",
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
    label: { type: "text", template: true, default: "Click me", show: { style: ["primary", "secondary", "success", "danger", "link"] }, description: "The text on the button." },
    style: { type: "select", pin: false, default: "primary", choices: STYLES, description: "The color of the button, Link to open a web page, or Premium to sell an item." },
    customId: { type: "text", default: "my-button", customIdOf: "button", label: "Custom id", description: "Identifies the button in Button Pressed.", show: { style: ["primary", "secondary", "success", "danger"] } },
    url: { type: "text", default: "https://", label: "URL", show: { style: ["link"] }, description: "The web page a Link button opens." },
    sku: { type: "text", default: "", label: "SKU id", show: { style: ["premium"] }, description: "The item it sells (Premium › List SKUs). Discord draws the button itself, with the item's name and price." },
    emoji: { type: "text", default: "", advanced: true, show: { style: ["primary", "secondary", "success", "danger", "link"] }, description: "An emoji such as 👍 or <:name:id>." },
    disabled: { type: "boolean", default: false, advanced: true, description: "Shown grayed out, so nobody can press it." },
  },
  outputs: {
    button: { type: "discord.component", description: "Connect to Send Message › Components." },
  },
  run({ fields }) {
    const { ButtonBuilder, ButtonStyle } = require("discord.js");
    if (fields.style === "premium") return { button: new ButtonBuilder().setStyle(ButtonStyle.Premium).setSKUId(String(fields.sku).trim()).setDisabled(Boolean(fields.disabled)) };
    const style = { primary: ButtonStyle.Primary, secondary: ButtonStyle.Secondary, success: ButtonStyle.Success, danger: ButtonStyle.Danger, link: ButtonStyle.Link }[fields.style];
    const b = new ButtonBuilder().setStyle(style).setLabel(String(fields.label || " ").slice(0, 80)).setDisabled(Boolean(fields.disabled));
    if (fields.style === "link") b.setURL(fields.url);
    else b.setCustomId(String(fields.customId || "button").slice(0, 100));
    if (fields.emoji) b.setEmoji(fields.emoji);
    return { button: b };
  },
};
