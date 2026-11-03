const SOURCES = { object: "A connected object", client: "The Discord client (the bot)" };
const MODES = { call: "Call a method", read: "Read a property", write: "Set a property" };

/** "123", "true", "{\"a\":1}" become numbers, yes/no and objects; other text stays text. */
function parseArg(line) {
  try {
    return JSON.parse(line);
  } catch {
    return line;
  }
}

module.exports = {
  id: "core.use_method",
  kind: "action",
  info: {
    name: "Use Method",
    description: "Calls any method, or reads or sets any property, of an object or of the Discord client: anything the library can do. Such as members.fetch, setNickname or guild.rulesChannel.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "code",
    aliases: ["discord.js", "Call method", "Methode aufrufen"],
    tags: ["call", "method", "property", "discord.js", "advanced", "api"],
  },
  look: {
    icon: "function",
  },
  fields: {
    source: { type: "select", pin: false, default: "object", choices: SOURCES, description: "What to start from." },
    object: { type: "any", pin: "only", required: true, show: { source: ["object"] }, description: "Any object, such as a member, channel or message." },
    mode: { type: "select", pin: false, default: "call", choices: MODES, description: "Call, read or set." },
    path: { type: "text", template: false, required: true, default: "fetch", description: "The method or property, with dots for deeper ones: guild.members.fetch" },
    args: { type: "textarea", default: "", show: { mode: ["call"] }, label: "Arguments", description: "One per line. Numbers, true/false and {JSON} are read as such; anything else is text." },
    values: { type: "list<any>", pin: "only", multi: true, show: { mode: ["call"] }, label: "More arguments", description: "Connected values, added after the typed ones." },
    value: { type: "any", pin: true, show: { mode: ["write"] }, description: "The new value." },
  },
  outputs: {
    result: { type: "any", description: "What the method returned (awaited), or the property's value." },
  },
  async run({ fields: f, ctx }) {
    let target = f.source === "client" ? ctx.discord?.client : f.object;
    const parts = String(f.path).split(".").map((p) => p.trim()).filter(Boolean);
    const last = parts.pop();
    for (const p of parts) {
      if (target == null) throw new Error(`Nothing at "${p}" in ${f.path}`);
      target = await target[p];
    }
    if (target == null) throw new Error(`Nothing to use "${last}" on`);
    if (f.mode === "read") return { result: await target[last] };
    if (f.mode === "write") {
      target[last] = f.value;
      return { result: f.value };
    }
    if (typeof target[last] !== "function") throw new Error(`${f.path} isn't a method`);
    const args = [...String(f.args || "").split(/\r?\n/).filter((l) => l.trim() !== "").map(parseArg), ...(f.values || [])];
    return { result: await target[last](...args) };
  },
};
