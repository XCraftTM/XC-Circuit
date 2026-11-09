module.exports = {
  id: "discord.on_pins_update",
  kind: "event",
  info: {
    name: "Pins Changed",
    description: "Runs when a message is pinned or unpinned in a channel. Discord doesn't say which one; Get Messages or Message Info can look.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "messages",
    aliases: ["Message pinned", "Message unpinned", "Pin changed"],
    tags: ["pin", "unpin"],
  },
  look: {
    icon: "message",
  },
  requires: {
    intents: ["Guilds", "GuildMessages"],
  },
  fields: {},
  outputs: {
    channel: { type: "discord.channel", description: "The channel." },
    server: { type: "discord.guild?", description: "The server." },
    lastPin: { type: "text", label: "Last pin", description: "When the newest pin was made (ISO date), empty when none is left." },
  },
  listen(ctx, emit) {
    ctx.discord.client.on("channelPinsUpdate", (channel, time) => emit({ channel, lastPin: time ? new Date(time).toISOString() : "", server: channel.guild ?? null }));
  },
};
