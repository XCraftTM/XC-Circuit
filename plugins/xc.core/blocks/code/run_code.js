const namesOf = (text, taken = []) => String(text || "").split(/[\r\n,]+/).map((s) => s.trim()).filter((s) => /^[A-Za-z_$][\w$]*$/.test(s) && !taken.includes(s));

const OWN = ["code", "inputs", "outputs"];

module.exports = {
  id: "core.run_code",
  kind: "action",
  info: {
    name: "Run Code",
    description: "Runs your own JavaScript, for anything the other blocks don't do. Inputs arrive as inputs.name; return an object with your outputs. ctx gives the bot (ctx.discord.client), stored data (ctx.store), variables and more.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "code",
    aliases: ["JavaScript", "Script", "Eval"],
    tags: ["javascript", "script", "custom", "advanced", "eval"],
  },
  look: {
    icon: "terminal",
  },
  fields: {
    inputs: { type: "text", pin: false, template: false, default: "a, b", description: "Input names, comma separated. Each gets an input to connect or type a value." },
    outputs: { type: "text", pin: false, template: false, default: "result", description: "Output names, comma separated. Return them: return { result: … }." },
    code: { type: "textarea", pin: false, template: false, default: "return { result: inputs.a + inputs.b };", description: "JavaScript (async: await works). Arguments: inputs, ctx, require." },
  },
  define({ values }) {
    const add = {};
    for (const k of namesOf(values.inputs, OWN)) add[k] = { type: "any", default: "", label: k, description: `inputs.${k} in the code.` };
    const outputs = {};
    for (const k of namesOf(values.outputs, OWN)) outputs[k] = { type: "any", label: k, description: `What the code returns as ${k}.` };
    return { add, outputs };
  },
  async run({ fields, ctx }) {
    const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
    const inputs = {};
    for (const k of namesOf(fields.inputs, OWN)) inputs[k] = fields[k];
    const fn = new AsyncFunction("inputs", "ctx", "require", String(fields.code || ""));
    const out = await fn(inputs, ctx, ctx.require || require);
    const names = namesOf(fields.outputs, OWN);
    if (out && typeof out === "object" && !Array.isArray(out)) return out;
    return names.length ? { [names[0]]: out } : {};
  },
};
