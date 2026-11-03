const ACTIONS = { archive: "Archive (hide)", unarchive: "Unarchive", lock: "Lock (only mods can write)", unlock: "Unlock", add: "Add a member", remove: "Remove a member", join: "The bot joins", leave: "The bot leaves", rename: "Rename" };

module.exports = {
  id: "discord.thread_action",
  kind: "action",
  info: {
    name: "Thread Action",
    description: "Archives, locks, renames a thread, or adds and removes members (for private threads).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    tags: ["archive", "lock", "close", "add member", "ticket"],
  },
  look: {
    icon: "message",
  },
  fields: {
    thread: { type: "discord.channel", pin: "only", required: true, description: "The thread." },
    action: { type: "select", pin: false, default: "archive", choices: ACTIONS, description: "What to do." },
    user: { type: "any", pin: "only", required: true, show: { action: ["add", "remove"] }, description: "A user or member (or id)." },
    name: { type: "text", default: "", show: { action: ["rename"] }, description: "The new name." },
    reason: { type: "text", default: "", advanced: true, description: "Shown in the audit log." },
  },
  outputs: {
    thread: { type: "discord.channel", description: "The same thread." },
  },
  async run({ fields: f }) {
    const t = f.thread;
    if (typeof t.isThread !== "function" || !t.isThread()) throw new Error("That channel isn't a thread");
    const reason = f.reason || undefined;
    const id = f.user?.id ?? String(f.user ?? "");
    const act = {
      archive: () => t.setArchived(true, reason),
      unarchive: () => t.setArchived(false, reason),
      lock: () => t.setLocked(true, reason),
      unlock: () => t.setLocked(false, reason),
      add: () => t.members.add(id),
      remove: () => t.members.remove(id),
      join: () => t.join(),
      leave: () => t.leave(),
      rename: () => t.setName(String(f.name || t.name).slice(0, 100), reason),
    }[f.action];
    await act();
    return { thread: t };
  },
};
