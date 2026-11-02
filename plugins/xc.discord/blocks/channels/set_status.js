const ACTIVITY = { Playing: "Playing", Watching: "Watching", Listening: "Listening to", Competing: "Competing in", Custom: "Custom status" };
const STATUS = { online: "Online", idle: "Idle", dnd: "Do not disturb", invisible: "Invisible" };

module.exports = {
  id: "discord.set_status",
  name: "Set Bot Status",
  description: "Changes the bot's status and activity, such as “Watching 3 servers”.",
  author: "XC Team",
  version: "1.0.0",
  category: "channels",
  icon: "bolt",
  kind: "action",
  tags: ["presence", "activity", "playing"],
  fields: {
    status: { type: "select", pin: false, default: "online", choices: STATUS, description: "Online, idle, do not disturb or invisible." },
    activity: { type: "select", pin: false, default: "Watching", choices: ACTIVITY, description: "The word shown before the text, such as Watching or Playing." },
    text: { type: "text", template: true, default: "the server", description: "The activity text. {placeholders} work." },
  },
  run({ fields, ctx }) {
    const { ActivityType } = require("discord.js");
    ctx.discord.client.user.setPresence({
      status: fields.status,
      activities: fields.text ? [{ name: fields.text, state: fields.activity === "Custom" ? fields.text : undefined, type: ActivityType[fields.activity] }] : [],
    });
  },
};
