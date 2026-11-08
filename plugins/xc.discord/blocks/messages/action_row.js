module.exports = {
  id: "discord.action_row",
  kind: "value",
  info: {
    name: "Action Row",
    description: "One row of a message's buttons (up to 5) or a select menu, so you choose which share a row. Without it, Send Message fills rows in order.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "components",
    aliases: ["Row", "Button row"],
    tags: ["component", "row", "buttons", "layout"],
  },
  look: {
    icon: "pointer",
  },
  fields: {
    components: {
      type: "list<discord.component>",
      pin: "only",
      multi: true,
      label: "Components",
      description: "Up to 5 buttons, or one select menu, from Button and Select Menu blocks: they show in this row, in the order connected.",
    },
  },
  outputs: {
    row: { type: "discord.component", description: "Connect to Send Message › Components: the row shows as it is." },
  },
  run({ fields }) {
    const { ActionRowBuilder } = require("discord.js");
    const parts = [fields.components || []].flat(2).filter(Boolean);
    if (!parts.length) throw new Error("An Action Row needs a button or a select menu: connect one to Components");
    if (parts.some((c) => c instanceof ActionRowBuilder)) throw new Error("An Action Row can't hold another row: connect buttons or a select menu");
    const menus = parts.filter((c) => typeof c.setMinValues === "function").length;
    if (menus && parts.length > 1) throw new Error(`A select menu takes a row of its own, but this row has ${parts.length} parts: give the menu its own Action Row`);
    if (parts.length > 5) throw new Error(`A row holds up to 5 buttons, but ${parts.length} are connected: split them over two Action Rows`);
    return { row: new ActionRowBuilder().addComponents(...parts) };
  },
};
