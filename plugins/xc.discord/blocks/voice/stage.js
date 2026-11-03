const ACTIONS = {
  start: "Start the stage (go live)",
  edit: "Change the topic",
  end: "End the stage",
  info: "Get the stage",
  speaker: "Make a member a speaker",
  audience: "Move a member to the audience",
  request: "The bot asks to speak",
};

module.exports = {
  id: "discord.stage",
  kind: "action",
  info: {
    name: "Stage",
    description: "Starts, changes and ends a stage in a stage channel, and moves members between speakers and audience. The bot needs Manage Channels, Mute Members and Move Members for the stage channel.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "voice",
    aliases: ["Start stage", "Stage instance", "Invite to speak"],
    tags: ["stage", "speaker", "audience", "live", "podcast"],
  },
  look: {
    icon: "bolt",
  },
  fields: {
    action: { type: "select", pin: false, default: "start", choices: ACTIONS, description: "What to do." },
    channel: { type: "discord.channel?", pin: "only", show: { action: ["start", "edit", "end", "info", "request"] }, label: "Stage channel", description: "The stage channel." },
    topic: { type: "text", default: "", show: { action: ["start", "edit"] }, description: "What the stage is about (1 to 120 characters)." },
    notify: { type: "boolean", pin: false, default: false, show: { action: ["start"] }, label: "Notify everyone", description: "Sends the server a notification that the stage started (needs Mention Everyone)." },
    member: { type: "discord.member?", pin: "only", show: { action: ["speaker", "audience"] }, description: "The member, who must be in the stage channel." },
  },
  outputs: {
    stage: { type: "object", description: "The stage instance (empty when it ended or none is live)." },
    topic: { type: "text", description: "Its topic." },
    live: { type: "boolean", description: "Whether a stage is live in the channel." },
  },
  async run({ fields: f, ctx }) {
    const out = (stage) => ({ stage: stage ?? null, topic: stage?.topic ?? "", live: Boolean(stage) });
    if (f.action === "speaker" || f.action === "audience") {
      if (!f.member?.voice?.channel) throw new Error("The member isn't in a stage channel");
      await f.member.voice.setSuppressed(f.action === "audience");
      return out(f.member.voice.channel.stageInstance);
    }
    const channel = f.channel;
    if (!channel?.guild) throw new Error("Connect a stage channel");
    const stages = channel.guild.stageInstances;
    if (f.action === "start") return out(await stages.create(channel, { topic: String(f.topic || "Stage").slice(0, 120), sendStartNotification: Boolean(f.notify) }));
    if (f.action === "edit") return out(await stages.edit(channel, { topic: String(f.topic).slice(0, 120) }));
    if (f.action === "end") {
      await stages.delete(channel);
      return out(null);
    }
    if (f.action === "request") {
      const me = channel.guild.members.me ?? (await channel.guild.members.fetchMe());
      await me.voice.setRequestToSpeak(true);
      return out(channel.stageInstance);
    }
    return out(await stages.fetch(channel).catch(() => null));
  },
};
