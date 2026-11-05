const ACTIONS = { get: "Get it", set: "Change it" };

/** "channel id | description | emoji" lines to welcome channels. */
function parseChannels(text) {
  return String(text || "")
    .split(/\r?\n/)
    .map((line) => line.split("|").map((s) => s.trim()))
    .filter(([channel]) => channel)
    .map(([channel, description = "", emoji = ""]) => ({ channel: channel.replace(/[<#>]/g, ""), description: description.slice(0, 42), emoji: emoji || undefined }));
}

module.exports = {
  id: "discord.welcome_screen",
  kind: "action",
  info: {
    name: "Welcome Screen",
    description: "Reads or changes the welcome screen new members see in a Community server: its description and up to 5 recommended channels. The bot needs Manage Server.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Server welcome screen", "Recommended channels"],
    tags: ["welcome", "community", "onboarding", "new members"],
  },
  look: {
    icon: "user-plus",
  },
  requires: {
    permissions: ["ManageGuild"],
  },
  fields: {
    action: { type: "select", pin: false, default: "get", choices: ACTIONS, description: "What to do." },
    server: { type: "discord.guild", pin: "only", required: true, description: "The server (it must be a Community server)." },
    enabled: { type: "boolean", pin: false, default: true, show: { action: ["set"] }, description: "Whether new members see it." },
    description: { type: "text", default: "", show: { action: ["set"] }, description: "The server description shown on it (up to 140 characters). Empty = keep." },
    channels: {
      type: "textarea",
      default: "",
      show: { action: ["set"] },
      placeholder: "123456789012345678 | Read the rules | 📜",
      description: "Up to 5 channels, one per line: channel id | what to do there | emoji. Empty = keep.",
    },
  },
  outputs: {
    enabled: { type: "boolean", description: "Whether it is on." },
    description: { type: "text", description: "Its description." },
    channels: { type: "list<object>", description: "Each recommended channel: channelId, description, emoji." },
  },
  async run({ fields: f }) {
    const s = f.server;
    if (!s?.fetchWelcomeScreen) throw new Error("Connect a server");
    let screen;
    if (f.action === "set") {
      const edit = { enabled: Boolean(f.enabled) };
      if (f.description) edit.description = String(f.description).slice(0, 140);
      const channels = parseChannels(f.channels);
      if (channels.length) edit.welcomeChannels = channels.slice(0, 5);
      screen = await s.editWelcomeScreen(edit);
    } else {
      screen = await s.fetchWelcomeScreen();
    }
    return {
      enabled: Boolean(screen.enabled),
      description: screen.description ?? "",
      channels: [...screen.welcomeChannels.values()].map((c) => ({ channelId: c.channelId, description: c.description, emoji: c.emoji?.toString() ?? "" })),
    };
  },
};
