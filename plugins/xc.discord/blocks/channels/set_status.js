const ACTIVITY = { Playing: "Playing", Watching: "Watching", Listening: "Listening to", Competing: "Competing in", Streaming: "Streaming", Custom: "Custom status", none: "No activity" };
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
    url: { type: "text", default: "https://twitch.tv/", show: { activity: ["Streaming"] }, label: "Stream URL", description: "A Twitch or YouTube link (needed for the purple Streaming status)." },
    afk: { type: "boolean", default: false, advanced: true, label: "AFK", description: "Mark the bot as away from keyboard." },
  },
  run({ fields, ctx }) {
    const { ActivityType } = require("discord.js");
    const activity = { name: fields.text, state: fields.activity === "Custom" ? fields.text : undefined, type: ActivityType[fields.activity] };
    if (fields.activity === "Streaming") activity.url = fields.url;
    ctx.discord.client.user.setPresence({
      status: fields.status,
      afk: Boolean(fields.afk),
      activities: fields.text && fields.activity !== "none" ? [activity] : [],
    });
  },
};
