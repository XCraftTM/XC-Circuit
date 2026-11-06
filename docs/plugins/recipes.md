# Recipes

::: info In short
Complete examples for things plugins often do. Copy one into your plugin, change the ids to your plugin's, and save: it loads right away. Blocks go into a file listed under `[contributes] blocks`; editor code goes into the `editor.js` named under `[editor]`.
:::

Each recipe says where its code goes. The ids start with `acme.`: use your plugin's id instead.

## Blocks

### A value block

Turns its input into something else; no action wires. `blocks/initials.js`:

```js
module.exports = {
  id: "acme.initials",
  kind: "value",
  info: { name: "Initials", description: "The first letter of each word, in capitals: Ada Lovelace → AL.", author: "Acme", version: "1.0.0" },
  search: { category: "text", aliases: ["Monogram"] },
  look: { icon: "text" },
  fields: { text: { type: "text", pin: true } },
  outputs: { initials: { type: "text" } },
  run({ fields }) {
    const initials = String(fields.text).split(/\s+/).filter(Boolean).map((w) => w[0].toUpperCase()).join("");
    return { initials };
  },
};
```

### An action with a choice

An action runs in a flow and continues on its action output. A `select` field gives a dropdown, and `show` hides fields that don't apply. `blocks/count.js`:

```js
module.exports = {
  id: "acme.count",
  kind: "action",
  info: { name: "Count", description: "Adds one to a counter kept for the bot or for a member, and gives the new count.", author: "Acme", version: "1.0.0" },
  search: { category: "data" },
  look: { icon: "hash" },
  fields: {
    scope: { type: "select", choices: { bot: "Whole bot", user: "Per user" }, default: "bot" },
    user: { type: "discord.user", pin: "only", show: { scope: ["user"] } },
    key: { type: "text", default: "count" },
  },
  outputs: { count: { type: "number" } },
  async run({ fields, ctx }) {
    const store = fields.scope === "user" ? ctx.store.user(fields.user.id) : ctx.store.global();
    return { count: await store.inc(fields.key) };
  },
};
```

### Branching

`flows` gives an action several ways out; [`ctx.next`](/api/ctx#next) picks one. `blocks/chance.js`:

```js
module.exports = {
  id: "acme.chance",
  kind: "action",
  info: { name: "Chance", description: "Goes on through Hit with the given chance, otherwise through Miss.", author: "Acme", version: "1.0.0" },
  search: { category: "flow", aliases: ["Random", "Probability"] },
  look: { icon: "branch" },
  fields: { percent: { type: "number", default: 50, min: 0, max: 100 } },
  flows: ["hit", "miss"],
  async run({ fields, ctx }) {
    await ctx.next(Math.random() * 100 < fields.percent ? "hit" : "miss");
  },
};
```

### Calling a web API

Use [`ctx.util.fetch`](/api/ctx#util): it limits the size of the answer and, for addresses the bot's users typed, keeps to the public web. `blocks/joke.js`:

```js
module.exports = {
  id: "acme.joke",
  kind: "action",
  info: { name: "Get Joke", description: "Gets a random joke from the web.", author: "Acme", version: "1.0.0" },
  search: { category: "web" },
  look: { icon: "message" },
  outputs: { joke: { type: "text" } },
  async run({ ctx }) {
    const { res, body } = await ctx.util.fetch("https://official-joke-api.appspot.com/random_joke", { headers: { accept: "application/json" } });
    if (!res.ok) throw new Error(`The joke service answered ${res.status}`);
    const j = JSON.parse(body.toString("utf8"));
    return { joke: `${j.setup} ${j.punchline}` };
  },
};
```

A thrown `Error` fails the block: it turns red, and the message goes to the Console (or to a [Try Zone](/explanations/errors)).

### An npm package

List it under `requires.dependencies`; XC Circuit installs it before Run and puts it into exports. Load it with [`ctx.require`](/api/ctx#require). `blocks/slug.js`:

```js
module.exports = {
  id: "acme.slug",
  kind: "value",
  info: { name: "Slug", description: "A text as a web-address-friendly slug: Hello World! → hello-world.", author: "Acme", version: "1.0.0" },
  search: { category: "text" },
  requires: { dependencies: { slugify: "^1.6.6" } },
  fields: { text: { type: "text", pin: true } },
  outputs: { slug: { type: "text" } },
  run({ fields, ctx }) {
    const slugify = ctx.require("slugify");
    return { slug: slugify(String(fields.text), { lower: true, strict: true }) };
  },
};
```

### Keeping data apart

Stored data your blocks keep can go into a namespace of your own, so it can't clash with the bot's own keys: [`ctx.store.ns`](/api/ctx#store).

```js
const seen = ctx.store.ns("acme.tools");
await seen.set(`last:${fields.user.id}`, Date.now());
```

It shows in Stored Data under `ns:acme.tools`.

## The editor

### A command

In the Plugins menu and the command palette (Ctrl+P). `editor.js`:

```js
xc.command({
  id: "acme.count-logs",
  title: "Count Log blocks",
  run() {
    const logs = xc.blocks().filter((b) => b.type === "core.log");
    xc.success(`${logs.length} Log blocks in ${xc.project().name}`);
  },
});
```

### A check for the Problems panel

Runs about once a second. A problem that names a block selects it when clicked.

```js
xc.check(() =>
  xc.blocks()
    .filter((b) => b.type === "core.log" && !b.values.message)
    .map((b) => ({ message: "This Log has no message", workspace: b.workspace, block: b.id })),
);
```

### A panel that lists and jumps

`render` runs again when the project changes, so it's always current.

```js
xc.panel({
  id: "acme.todo",
  title: "TODO notes",
  render(ui) {
    const todos = xc.workspaces().flatMap((w) =>
      Object.values(xc.workspace(w.key).notes ?? {})
        .filter((n) => /TODO/i.test(n.text ?? ""))
        .map((n) => ({ workspace: w, note: n })),
    );
    if (!todos.length) return ui.muted("No notes with TODO.");
    for (const t of todos) {
      ui.row((ui) => {
        ui.text(t.workspace.name, { bold: true });
        ui.text(t.note.text);
      });
    }
  },
});
```

### Settings for your plugin

Panels are *controlled*: keep what people type and give it back on the next render. [`xc.storage`](/api/editor#quick-reference) keeps it on this computer, between sessions.

```js
xc.panel({
  id: "acme.settings",
  title: "Acme settings",
  render(ui) {
    const s = xc.storage.get("settings") ?? { greeting: "Hello", loud: false };
    const save = (change) => xc.storage.set("settings", { ...s, ...change });
    ui.input(s.greeting, (greeting) => save({ greeting }), { placeholder: "Greeting", key: "greeting" });
    ui.toggle("Shout it", s.loud, (loud) => save({ loud }));
    ui.button("Try it", () => xc.log(s.loud ? s.greeting.toUpperCase() + "!" : s.greeting), { primary: true });
  },
});
```

For settings that belong to the project (and go with it to other computers), use `xc.data` with a key starting with your plugin's id.

### Adding blocks for someone

Changes go through undo like the person's own edits.

```js
xc.command({
  id: "acme.add-log",
  title: "Add a Log next to the selection",
  run() {
    const ws = xc.activeWorkspace();
    const [first] = xc.selection();
    if (!ws || !first) return xc.warn("Select a block first.");
    const at = xc.workspace(ws).nodes[first].at ?? [0, 0];
    const id = xc.addBlock(ws, "core.log", { x: at[0] + 260, y: at[1], values: { message: "Here!" } });
    xc.select(ws, [id]);
  },
});
```

### Reacting to the running bot

`xc.on("botEvent")` gets what the bot run from the editor does: starts, stops, logs, and every block that runs (keep that quick).

```js
let failed = 0;
xc.on("botEvent", (e) => {
  if (e.type === "started") failed = 0;
  if (e.type === "block" && e.error) failed += 1;
});
xc.panel({
  id: "acme.health",
  title: "Bot health",
  render(ui) {
    ui.stat("failed blocks since start", failed);
    ui.button(xc.botRunning() ? "Stop" : "Run", () => xc.run(xc.botRunning() ? "bot.stop" : "bot.run"));
  },
});
```

### Something on a timer

```js
let ticks = 0;
xc.every(60_000, () => {
  ticks += 1;
  if (ticks % 30 === 0) xc.log("Half an hour of work: time for a break?");
});
```

## More

- DemoBot's `plugins/demo.custom/`: a block, a panel, a command and a check together.
- [Desk Pet](https://github.com/XCraftTM/XC-Circuit/tree/main/examples/plugins/desk-pet): an overlay with pictures, sounds and timers.
- The bundled plugins in the app's `plugins/` folder: every block XC Circuit has, as ordinary plugin files.
