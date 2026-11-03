const ACTIONS = { get: "Get it", set: "Change it" };
const MODES = { keep: "Keep as it is", default: "Default channels only count", advanced: "Questions count too" };

module.exports = {
  id: "discord.onboarding",
  kind: "action",
  info: {
    name: "Onboarding",
    description: "Reads or changes a Community server's onboarding: whether it is on, the default channels, and the questions new members answer (each answer can give roles and channels). The bot needs Manage Server and Manage Roles.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Server onboarding", "Onboarding questions", "Customization questions"],
    tags: ["onboarding", "community", "welcome", "questions", "roles"],
  },
  look: {
    icon: "user-plus",
  },
  fields: {
    action: { type: "select", pin: false, default: "get", choices: ACTIONS, description: "What to do." },
    server: { type: "discord.guild", pin: "only", required: true, description: "The server (it must be a Community server)." },
    enabled: { type: "boolean", pin: false, default: true, show: { action: ["set"] }, description: "Whether new members go through onboarding." },
    mode: { type: "select", pin: false, default: "keep", choices: MODES, show: { action: ["set"] }, description: "Which choices count toward Discord's minimum of 7 channels." },
    defaultChannels: { type: "list<discord.channel>", pin: "only", multi: true, show: { action: ["set"] }, label: "Default channels", description: "Channels every member gets. None connected = keep." },
    prompts: {
      type: "textarea",
      template: false,
      default: "",
      show: { action: ["set"] },
      description: "The questions as JSON, replacing all of them. Empty = keep. Such as [{ \"title\": \"What games?\", \"options\": [{ \"title\": \"Minecraft\", \"roles\": [\"<role id>\"], \"channels\": [\"<channel id>\"] }] }]",
    },
  },
  outputs: {
    enabled: { type: "boolean", description: "Whether onboarding is on." },
    mode: { type: "text", description: "default or advanced." },
    defaultChannels: { type: "list<discord.channel>", label: "Default channels", description: "The default channels." },
    prompts: { type: "list<object>", description: "Each question: title, required, singleSelect and its options (title, roles, channels)." },
  },
  async run({ fields: f }) {
    const s = f.server;
    if (!s?.fetchOnboarding) throw new Error("Connect a server");
    let onboarding;
    if (f.action === "set") {
      const edit = { enabled: Boolean(f.enabled) };
      if (f.mode !== "keep") edit.mode = f.mode === "advanced" ? 1 : 0;
      const channels = [f.defaultChannels].flat().filter(Boolean);
      if (channels.length) edit.defaultChannels = channels;
      if (String(f.prompts || "").trim()) {
        let prompts;
        try {
          prompts = JSON.parse(f.prompts);
        } catch (e) {
          throw new Error(`The questions aren't valid JSON: ${e.message}`);
        }
        edit.prompts = (Array.isArray(prompts) ? prompts : [prompts]).map((p) => ({ required: false, singleSelect: false, inOnboarding: true, ...p }));
      }
      onboarding = await s.editOnboarding(edit);
    } else {
      onboarding = await s.fetchOnboarding();
    }
    return {
      enabled: Boolean(onboarding.enabled),
      mode: onboarding.mode === 1 ? "advanced" : "default",
      defaultChannels: [...onboarding.defaultChannels.values()],
      prompts: [...onboarding.prompts.values()].map((p) => ({
        title: p.title,
        required: p.required,
        singleSelect: p.singleSelect,
        options: [...p.options.values()].map((o) => ({ title: o.title, roles: [...o.roles.keys()], channels: [...o.channels.keys()] })),
      })),
    };
  },
};
