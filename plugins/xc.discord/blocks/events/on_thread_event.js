const KINDS = { updated: "Changed", deleted: "Deleted", members: "Members join or leave" };

module.exports = {
  id: "discord.on_thread_event",
  kind: "event",
  info: {
    name: "Thread Changed / Deleted",
    description: "Runs when a thread or forum post is changed (renamed, archived, locked, tags), deleted, or when members join or leave it.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["Thread updated", "Thread deleted", "Thread archived", "Forum post changed"],
    tags: ["thread", "forum", "archive", "log"],
  },
  look: {
    icon: "hash",
  },
  requires: {
    intents: ["Guilds", "GuildMembers"],
  },
  fields: {
    kind: { type: "select", pin: false, default: "updated", label: "When", choices: KINDS, description: "Which event runs it." },
  },
  outputs: {
    thread: { type: "discord.channel", description: "The thread (as it is now, or was before deleting)." },
    beforeName: { type: "text", label: "Name before", description: "Its name before a change." },
    archived: { type: "boolean", description: "Whether it is archived now." },
    locked: { type: "boolean", description: "Whether it is locked now." },
    joined: { type: "list<discord.user>", description: "For member changes: who joined." },
    left: { type: "list<discord.user>", description: "For member changes: who left." },
    server: { type: "discord.guild", description: "The server." },
  },
  listen(ctx, emit) {
    const c = ctx.discord.client;
    const out = (thread, extra = {}) =>
      emit({ thread, beforeName: thread.name ?? "", archived: Boolean(thread.archived), locked: Boolean(thread.locked), joined: [], left: [], server: thread.guild, ...extra });
    const kind = ctx.fields.kind;
    if (kind === "updated") c.on("threadUpdate", (old, now) => out(now, { beforeName: old.name ?? "" }));
    else if (kind === "deleted") c.on("threadDelete", (thread) => out(thread));
    else
      c.on("threadMembersUpdate", (added, removed, thread) =>
        out(thread, { joined: [...added.values()].map((m) => m.user).filter(Boolean), left: [...removed.values()].map((m) => m.user).filter(Boolean) }),
      );
  },
};
