const ACTIONS = { create: "Create", edit: "Change", start: "Start now", end: "End", delete: "Delete", list: "List" };
const WHERE = { external: "Somewhere else (a place or link)", voice: "A voice channel", stage: "A stage channel" };

module.exports = {
  id: "discord.scheduled_event",
  name: "Scheduled Event",
  description: "Creates, changes, starts, ends, deletes or lists a server's scheduled events.",
  author: "XC Team",
  version: "1.0.0",
  category: "server",
  icon: "clock",
  kind: "action",
  tags: ["event", "calendar", "meetup", "stage"],
  fields: {
    action: { type: "select", pin: false, default: "create", choices: ACTIONS, description: "What to do." },
    server: { type: "discord.guild", pin: "only", required: true, show: { action: ["create", "list"] }, description: "The server." },
    event: { type: "object", pin: "only", required: true, show: { action: ["edit", "start", "end", "delete"] }, description: "The event (from List or Server Event)." },
    name: { type: "text", default: "Game night", show: { action: ["create", "edit"] }, description: "The event's name." },
    description: { type: "textarea", default: "", show: { action: ["create", "edit"] }, description: "What it's about." },
    start: { type: "any", pin: true, default: "", show: { action: ["create", "edit"] }, description: "When it starts: a date, or text such as 2026-12-24 18:00." },
    end: { type: "any", pin: true, default: "", show: { action: ["create", "edit"] }, description: "When it ends (needed for events somewhere else)." },
    where: { type: "select", pin: false, default: "external", choices: WHERE, show: { action: ["create"] }, description: "Where it happens." },
    location: { type: "text", default: "", show: { action: ["create", "edit"] }, description: "The place or link (for somewhere else)." },
    channel: { type: "discord.channel?", pin: "only", show: { action: ["create", "edit"] }, description: "The voice or stage channel." },
    image: { type: "text", default: "", advanced: true, label: "Cover image URL", show: { action: ["create", "edit"] }, description: "A picture for the event." },
  },
  outputs: {
    event: { type: "object", description: "The event." },
    events: { type: "list<object>", description: "For List: every scheduled event." },
    url: { type: "text", description: "A link to the event." },
  },
  async run({ fields: f }) {
    const d = require("discord.js");
    const date = (v) => (v === "" || v == null ? undefined : new Date(v));
    if (f.action === "list") {
      const all = [...(await f.server.scheduledEvents.fetch()).values()];
      return { events: all, event: all[0] ?? null, url: all[0]?.url ?? "" };
    }
    if (f.action === "delete") {
      await f.event.delete();
      return { event: f.event, events: [], url: "" };
    }
    if (f.action === "start" || f.action === "end") {
      const status = f.action === "start" ? d.GuildScheduledEventStatus.Active : d.GuildScheduledEventStatus.Completed;
      const e = await f.event.setStatus(status);
      return { event: e, events: [], url: e.url };
    }
    const data = {};
    if (f.name) data.name = String(f.name).slice(0, 100);
    if (f.description) data.description = String(f.description).slice(0, 1000);
    if (date(f.start)) data.scheduledStartTime = date(f.start);
    if (date(f.end)) data.scheduledEndTime = date(f.end);
    if (f.image) data.image = f.image;
    if (f.action === "edit") {
      if (f.location) data.entityMetadata = { location: f.location };
      if (f.channel) data.channel = f.channel;
      const e = await f.event.edit(data);
      return { event: e, events: [], url: e.url };
    }
    data.privacyLevel = d.GuildScheduledEventPrivacyLevel.GuildOnly;
    if (f.where === "external") {
      data.entityType = d.GuildScheduledEventEntityType.External;
      data.entityMetadata = { location: f.location || "Online" };
      if (!data.scheduledEndTime) throw new Error("Events somewhere else need an end time");
    } else {
      data.entityType = f.where === "stage" ? d.GuildScheduledEventEntityType.StageInstance : d.GuildScheduledEventEntityType.Voice;
      data.channel = f.channel;
    }
    if (!data.scheduledStartTime) throw new Error("Give the event a start time");
    const e = await f.server.scheduledEvents.create(data);
    return { event: e, events: [], url: e.url };
  },
};
