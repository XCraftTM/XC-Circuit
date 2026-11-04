# Your first block

Blocks are small JavaScript (or TypeScript) files. In this tutorial you write two: a **value** block, *Greeting*, that says "Good morning" or "Good evening" depending on the time, and an **action** block, *Coin Flip*, that continues on *Heads* or *Tails*. You put them in the project's own plugin, so they travel with the project.

## 1. A plugin inside the project

Every block belongs to a plugin. A project can have its own plugins in its `plugins/` folder. Open the project folder (from the bot in [Your first bot](./first-bot), or any project) and create:

```
Ping Bot/
├─ circuit.json
├─ workspaces/
└─ plugins/
   └─ my.blocks/
      ├─ plugin.toml
      └─ blocks/
```

`plugin.toml` says what the plugin is and where its blocks are:

```toml
id          = "my.blocks"
name        = "My blocks"
version     = "1.0.0"
author      = "You"
description = "Blocks for this bot."

[engines]
circuit = "^1.0"

[contributes]
blocks = ["blocks/**/*.{js,ts}"]
```

## 2. A value block

Create `plugins/my.blocks/blocks/greeting.js`:

```js
module.exports = {
  id: "my.greeting",
  kind: "value",
  info: {
    name: "Greeting",
    description: "Good morning, good afternoon or good evening, by the time of day.",
    author: "You",
    version: "1.0.0",
  },
  search: { category: "text", aliases: ["Hello", "Good morning"] },
  look: { icon: "message" },
  fields: {
    name: { type: "text", default: "", description: "Who to greet." },
  },
  outputs: {
    text: { type: "text", description: "The greeting." },
  },
  run({ fields }) {
    const hour = new Date().getHours();
    const part = hour < 12 ? "morning" : hour < 18 ? "afternoon" : "evening";
    return { text: `Good ${part}, ${fields.name || "you"}!` };
  },
};
```

Save it. XC Circuit watches plugin files and loads the block straight away; if something is wrong, the Problems panel says what.

- `kind: "value"` makes it a value block: it has no action wires, and it runs when an action needs its output.
- `fields` are the inputs you fill in on the block. Each can also become a pin to connect a value to.
- `run()` gets the fields' values and returns the outputs.

Press **Space** on the canvas and search for *Greeting*. Add it, connect its **Text** output to a Send Message's text (type `${` in the text and pick it, or connect it to a `${greeting}` input), and run the bot.

## 3. An action block with two ways out

Actions run in order along the green wires. An action with `flows` decides which way the flow goes on. Create `plugins/my.blocks/blocks/coin_flip.js`:

```js
module.exports = {
  id: "my.coin_flip",
  kind: "action",
  info: {
    name: "Coin Flip",
    description: "Continues on Heads or Tails, at random.",
    author: "You",
    version: "1.0.0",
  },
  search: { category: "flow", aliases: ["Random", "50/50"] },
  look: { icon: "branch" },
  flows: ["heads", "tails"],
  outputs: {
    result: { type: "text", description: "heads or tails." },
  },
  async run({ ctx }) {
    const result = Math.random() < 0.5 ? "heads" : "tails";
    await ctx.next(result, { result });
  },
};
```

`ctx.next(flow, outputs)` continues the flow on one of the block's `flows`, after setting its outputs. An action without `flows` simply returns its outputs, and the flow continues on its single action output by itself.

## 4. Optional: TypeScript

Rename the file to `coin_flip.ts` and add types. XC Circuit strips the types when it loads the block; for checking them in your code editor, add the [`@xc-circuit/sdk`](https://github.com/XCraftTM/XC-Circuit/tree/main/sdk) types:

```ts
import { defineBlock } from "@xc-circuit/sdk";

export default defineBlock({
  id: "my.coin_flip",
  kind: "action",
  // … the same as above
});
```

## What you learned

- Blocks live in plugins; a project's `plugins/` folder holds plugins that travel with it.
- A block has an `id`, a `kind`, `info`, `fields`, `outputs` and a `run()` function.
- Value blocks return outputs; actions continue with `ctx.next()` when they have several `flows`.

Every key a block can have is in the [Blocks reference](/api/blocks); what `run()` can use is in [Run context (ctx)](/api/ctx).
