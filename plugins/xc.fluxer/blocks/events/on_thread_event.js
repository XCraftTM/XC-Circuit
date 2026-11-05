const WHEN = { create: "Created", update: "Changed", delete: "Deleted" };

module.exports = {
  id: "fluxer.on_thread_event",
  kind: "event",
  info: {
    name: "Thread Created, Changed or Deleted",
    description: "Runs when a thread or forum post is made, changed (archived, locked, renamed …) or deleted.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["On thread", "Thread created", "Forum post created"],
  },
  look: {
    icon: "branch",
  },
  fields: {
    when: { type: "select", pin: false, default: "create", choices: WHEN, label: "When", description: "Which change." },
  },
  outputs: {
    thread: { type: "fluxer.thread", description: "The thread (as it is now, or was before deleting)." },
    before: { type: "fluxer.channel?", description: "For changes: the thread as it was." },
    name: { type: "text", description: "Its name." },
    parentId: { type: "text", label: "Parent id", description: "The channel or forum it is in." },
  },
  listen(ctx, emit) {
    const client = ctx.fluxer.client;
    const out = (thread, before = null) => emit({ thread, before, name: thread?.name ?? "", parentId: String(thread?.parentId ?? "") });
    if (ctx.fields.when === "update") client.on("threadUpdate", (before, thread) => out(thread, before));
    else client.on(ctx.fields.when === "delete" ? "threadDelete" : "threadCreate", (thread) => out(thread));
  },
};
