module.exports = {
  id: "core.for_each",
  kind: "action",
  info: {
    name: "For Each",
    description: "Runs Each once for every item in a list, then Done.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "flow",
    aliases: ["Loop list", "Iterate", "Für jedes", "Schleife"],
  },
  look: {
    icon: "repeat",
  },
  flows: ["each", "done"],
  fields: {
    list: { type: "list<any>", pin: "only", required: true, description: "The list to go through." },
  },
  outputs: {
    item: { type: "any", description: "The current item." },
    index: { type: "number", description: "Position of the current item, starting at 0." },
  },
  define({ connected }) {
    // The item output takes the element type of whatever list is connected.
    return { outputs: { item: { type: circuit.types.elementOf(connected.list), description: "The current item." } } };
  },
  async run({ fields, ctx }) {
    for (const [index, item] of (fields.list || []).entries()) {
      await ctx.next("each", { item, index });
    }
    await ctx.next("done");
  },
};
