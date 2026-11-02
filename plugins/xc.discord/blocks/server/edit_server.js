const LEVELS = { keep: "Keep as is", "0": "None", "1": "Low (verified email)", "2": "Medium (5 minutes on Discord)", "3": "High (10 minutes in the server)", "4": "Highest (verified phone)" };

module.exports = {
  id: "discord.edit_server",
  name: "Edit Server",
  description: "Changes the server's name, icon, banner, description, system channel or verification level. Empty fields stay as they are. The bot needs Manage Server.",
  author: "XC Team",
  version: "1.0.0",
  category: "server",
  icon: "layers",
  kind: "action",
  tags: ["guild settings", "rename server", "icon"],
  fields: {
    server: { type: "discord.guild", pin: "only", required: true, description: "The server." },
    name: { type: "text", default: "", description: "New name." },
    description: { type: "text", default: "", description: "New description (community servers)." },
    icon: { type: "text", default: "", label: "Icon URL", description: "New icon image." },
    banner: { type: "text", default: "", advanced: true, label: "Banner URL", description: "New banner (needs boosts)." },
    systemChannel: { type: "discord.channel?", pin: "only", label: "System channel", description: "Where join messages go." },
    verification: { type: "select", pin: false, default: "keep", choices: LEVELS, advanced: true, label: "Verification level", description: "What new members must have before they can write." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  outputs: {
    server: { type: "discord.guild", description: "The changed server." },
  },
  async run({ fields: f }) {
    const edit = { reason: f.reason || undefined };
    if (f.name) edit.name = String(f.name).slice(0, 100);
    if (f.description) edit.description = String(f.description);
    if (f.icon) edit.icon = f.icon;
    if (f.banner) edit.banner = f.banner;
    if (f.systemChannel) edit.systemChannel = f.systemChannel;
    if (f.verification !== "keep") edit.verificationLevel = Number(f.verification);
    return { server: await f.server.edit(edit) };
  },
};
