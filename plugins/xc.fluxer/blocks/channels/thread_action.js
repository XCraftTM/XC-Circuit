const ACTIONS = {
  archive: "Archive",
  unarchive: "Reopen",
  lock: "Lock (only mods can write)",
  unlock: "Unlock",
  rename: "Rename",
  join: "Bot joins",
  leave: "Bot leaves",
};

module.exports = {
  id: "fluxer.thread_action",
  kind: "action",
  info: {
    name: "Change Thread",
    description: "Archives, reopens, locks, unlocks or renames a thread, or lets the bot join or leave it.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "channels",
    aliases: ["Archive thread", "Lock thread", "Close thread", "Rename thread"],
  },
  look: {
    icon: "branch",
  },
  requires: {
    permissions: { action: { archive: ["ManageChannels"], unarchive: ["ManageChannels"], lock: ["ManageChannels"], unlock: ["ManageChannels"], rename: ["ManageChannels"] } },
  },
  fields: {
    action: { type: "select", pin: false, default: "archive", choices: ACTIONS, description: "What to do." },
    thread: { type: "fluxer.thread", pin: "only", required: true, description: "The thread." },
    name: { type: "text", default: "", show: { action: ["rename"] }, description: "The new name." },
    reason: { type: "text", default: "", advanced: true, description: "Why, for the audit log." },
  },
  outputs: {
    thread: { type: "fluxer.thread", description: "The same thread." },
  },
  async run({ fields: f }) {
    const t = f.thread;
    if (!t) throw new Error("Connect the thread");
    const reason = String(f.reason ?? "").trim() || undefined;
    switch (f.action) {
      case "archive":
        return { thread: await t.setArchived(true, reason) };
      case "unarchive":
        return { thread: await t.setArchived(false, reason) };
      case "lock":
        return { thread: await t.setLocked(true, reason) };
      case "unlock":
        return { thread: await t.setLocked(false, reason) };
      case "rename": {
        const name = String(f.name ?? "").trim();
        if (!name) throw new Error("Give the new name");
        return { thread: await t.edit({ name: name.slice(0, 100) }) };
      }
      case "join":
        await t.join();
        return { thread: t };
      case "leave":
        await t.leave();
        return { thread: t };
      default:
        throw new Error(`Unknown action ${f.action}`);
    }
  },
};
