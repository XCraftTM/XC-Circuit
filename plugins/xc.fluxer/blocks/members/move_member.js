module.exports = {
  id: "fluxer.move_member",
  kind: "action",
  info: {
    name: "Move in Voice",
    description: "Moves someone to another voice channel, or out of voice when no channel is given.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "calls",
    aliases: ["Move member", "Disconnect from voice", "Kick from voice"],
  },
  look: {
    icon: "volume",
  },
  requires: {
    permissions: ["MoveMembers"],
  },
  fields: {
    who: { type: "any", pin: true, required: true, description: "A member, or a user or id together with the server." },
    server: { type: "fluxer.server", pin: "only", description: "The server. Needed when Who is a user or an id." },
    channel: { type: "any", pin: true, description: "The voice channel to move them to (or its id). Empty = out of voice." },
  },
  outputs: {
    member: { type: "fluxer.member", description: "The same member." },
  },
  async run({ fields: f, ctx }) {
    const member = await ctx.fluxer.member(f.who, f.server);
    const channelId = f.channel ? ctx.fluxer.id(f.channel, "channel") : null;
    await member.move(channelId);
    return { member };
  },
};
