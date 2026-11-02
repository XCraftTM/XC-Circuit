module.exports = {
  id: "discord.on_thread_create",
  name: "Thread Created",
  description: "Runs when a thread or forum post is created.",
  author: "XC Team",
  version: "1.0.0",
  category: "events",
  icon: "message",
  kind: "event",
  intents: ["Guilds"],
  tags: ["forum post", "thread"],
  fields: {
    parent: { type: "text", pin: false, default: "", label: "In channel id", description: "Only threads in this channel or forum. Empty = any." },
  },
  outputs: {
    thread: { type: "discord.channel", description: "The new thread." },
    parent: { type: "discord.channel?", description: "The channel or forum it is in." },
    ownerId: { type: "text", label: "Creator id", description: "Who created it." },
    name: { type: "text", description: "Its name." },
  },
  listen(ctx, emit) {
    const only = String(ctx.fields.parent || "").trim();
    ctx.discord.client.on("threadCreate", (thread, newlyCreated) => {
      if (!newlyCreated || (only && thread.parentId !== only)) return;
      emit({ thread, parent: thread.parent ?? null, ownerId: thread.ownerId ?? "", name: thread.name });
    });
  },
};
