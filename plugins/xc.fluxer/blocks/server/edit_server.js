module.exports = {
  id: "fluxer.edit_server",
  kind: "action",
  info: {
    name: "Change Server",
    description: "Changes a server's name, icon, banner or system channel. Empty fields stay as they are.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Edit server", "Rename server", "Server icon"],
  },
  look: {
    icon: "shield",
  },
  requires: {
    permissions: ["ManageGuild"],
  },
  fields: {
    server: { type: "fluxer.server", pin: "only", required: true, description: "The server." },
    name: { type: "text", default: "", description: "Its new name." },
    icon: { type: "text", default: "", description: "A link to the new icon." },
    banner: { type: "text", default: "", advanced: true, description: "A link to the new banner." },
    systemChannel: { type: "fluxer.channel", pin: "only", label: "System channel", description: "Where join messages go." },
  },
  outputs: {
    server: { type: "fluxer.server", description: "The changed server." },
  },
  async run({ fields: f, ctx }) {
    if (!f.server) throw new Error("Connect the server");
    const options = {};
    const name = String(f.name ?? "").trim();
    if (name) options.name = name.slice(0, 100);
    const icon = await ctx.fluxer.imageData(f.icon);
    if (icon) options.icon = icon;
    const banner = await ctx.fluxer.imageData(f.banner);
    if (banner) options.banner = banner;
    if (f.systemChannel) options.systemChannelId = String(f.systemChannel.id ?? ctx.fluxer.id(f.systemChannel, "channel"));
    if (!Object.keys(options).length) return { server: f.server };
    return { server: await f.server.edit(options) };
  },
};
