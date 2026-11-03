module.exports = {
  id: "fluxer.server_info",
  kind: "value",
  info: {
    name: "Server Info",
    description: "A server's name, id, member count, owner and icon.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Guild info", "Member count", "Server name"],
  },
  look: {
    icon: "layers",
  },
  fields: {
    server: { type: "fluxer.server", pin: "only", required: true, description: "The server." },
  },
  outputs: {
    name: { type: "text", description: "Its name." },
    id: { type: "text", description: "Its id." },
    members: { type: "number", description: "How many members it has, when Fluxer said so." },
    ownerId: { type: "text", label: "Owner id", description: "The owner's id." },
    icon: { type: "text", description: "Its icon's web address. Empty when it has none." },
    createdAt: { type: "date", label: "Created at", description: "When it was made." },
    systemChannelId: { type: "text", label: "System channel id", description: "Where Fluxer posts join messages. Empty when not set." },
  },
  run({ fields }) {
    const s = fields.server;
    if (!s) throw new Error("Connect the server");
    return {
      name: s.name ?? "",
      id: s.id,
      members: s.memberCount ?? 0,
      ownerId: s.ownerId ?? "",
      icon: s.iconURL?.() ?? "",
      createdAt: s.createdAt ?? null,
      systemChannelId: s.systemChannelId ?? "",
    };
  },
};
