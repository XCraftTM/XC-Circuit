# Your first block

A **Greeting** block that says "Good morning", "Good afternoon" or "Good evening". Blocks are small JavaScript files.

## 1. Make a plugin in your project

In your bot's folder, create these folders and files:

```
Ping Bot/
└─ plugins/
   └─ my.blocks/
      ├─ plugin.toml
      └─ blocks/
         └─ greeting.js
```

`plugin.toml`:

```toml
id      = "my.blocks"
name    = "My blocks"
version = "1.0.0"

[contributes]
blocks = ["blocks/*.js"]
```

## 2. Write the block

`blocks/greeting.js`:

```js
module.exports = {
  id: "my.greeting",
  kind: "value",
  info: { name: "Greeting", description: "Good morning, afternoon or evening.", author: "You", version: "1.0.0" },
  fields: { name: { type: "text" } },
  outputs: { text: { type: "text" } },
  run({ fields }) {
    const hour = new Date().getHours();
    const part = hour < 12 ? "morning" : hour < 18 ? "afternoon" : "evening";
    return { text: `Good ${part}, ${fields.name || "you"}!` };
  },
};
```

Save. XC Circuit loads it right away.

## 3. Use it

Right-click the canvas:

![The picker finds the new block](/screens/greeting-picker.png)

1. Type `greet`.
2. There it is. Click it, and connect its **Text** to a Send Message.

## How it works

| Key | What it is |
| --- | --- |
| `kind: "value"` | gives a value when another block needs it (others: `"action"`, `"event"`) |
| `fields` | what you fill in on the block |
| `outputs` | what it gives |
| `run()` | gets the fields, returns the outputs |

::: details An action with two ways out
```js
module.exports = {
  id: "my.coin_flip",
  kind: "action",
  info: { name: "Coin Flip", description: "Heads or tails.", author: "You", version: "1.0.0" },
  flows: ["heads", "tails"],
  run({ ctx }) {
    return ctx.next(Math.random() < 0.5 ? "heads" : "tails");
  },
};
```
`flows` are the green outputs; `ctx.next()` picks one.
:::

Everything a block can have: [Blocks](/api/blocks).
