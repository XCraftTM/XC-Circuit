const ACTIONS = { list: "List the server's templates", create: "Create a template", sync: "Update a template to the server as it is now", delete: "Delete a template", info: "Look up a template by code" };

const info = (t) => ({ code: t.code, name: t.name, description: t.description ?? "", url: t.url ?? `https://discord.new/${t.code}`, uses: t.usageCount ?? 0, outOfDate: Boolean(t.unSynced) });

module.exports = {
  id: "discord.server_templates",
  kind: "action",
  info: {
    name: "Server Templates",
    description: "Creates, lists, updates and deletes a server's templates (a copy of its channels, roles and settings that others can make a server from), or looks one up by its code. The bot needs Manage Server.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "server",
    aliases: ["Guild template", "Create template", "Template link"],
    tags: ["template", "copy", "backup", "discord.new"],
  },
  look: {
    icon: "copy",
  },
  requires: {
    permissions: ["ManageGuild"],
  },
  fields: {
    action: { type: "select", pin: false, default: "list", choices: ACTIONS, description: "What to do." },
    server: { type: "discord.guild?", pin: "only", show: { action: ["list", "create", "sync", "delete"] }, description: "The server." },
    code: { type: "text", default: "", show: { action: ["sync", "delete", "info"] }, description: "The template's code or link (discord.new/…). For sync and delete: empty = the server's only template." },
    name: { type: "text", default: "", show: { action: ["create"] }, description: "The template's name." },
    description: { type: "text", default: "", show: { action: ["create"] }, description: "What it is for (optional)." },
  },
  outputs: {
    template: { type: "object", description: "The template." },
    url: { type: "text", description: "Its link." },
    code: { type: "text", description: "Its code." },
    list: { type: "list<object>", description: "For lists: each code, name, description, url, uses and outOfDate." },
  },
  async run({ fields: f, ctx }) {
    const none = { template: null, url: "", code: "", list: [] };
    const one = (t) => ({ ...none, template: t, url: info(t).url, code: t.code });
    const code = String(f.code || "").trim().replace(/^.*\//, "");
    if (f.action === "info") return one(await ctx.discord.client.fetchGuildTemplate(code));
    const s = f.server;
    if (!s?.fetchTemplates) throw new Error("Connect a server");
    if (f.action === "create") return one(await s.createTemplate(String(f.name || s.name).slice(0, 100), f.description || undefined));
    const all = await s.fetchTemplates();
    if (f.action === "list") return { ...none, list: [...all.values()].map(info) };
    const t = code ? all.get(code) : all.first();
    if (!t) throw new Error(code ? `The server has no template ${code}` : "The server has no template");
    if (f.action === "delete") {
      await t.delete();
      return none;
    }
    return one(await t.sync());
  },
};
