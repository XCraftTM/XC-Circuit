# Blocks

::: info In short
A block is one JavaScript or TypeScript file that exports an object: an `id`, a `kind` (`"event"`, `"action"` or `"value"`), `info` (name, description…), `fields` (what you fill in), `outputs` (what it gives) and `run()` (what it does).
:::

A block is one `.js`, `.cjs`, `.mjs` or `.ts` file that exports an object. Files are found through the plugin's `contributes.blocks` patterns.

```js
module.exports = {
  id: "acme.shout",
  kind: "value",
  info: { name: "Shout", description: "Upper-cases a text.", author: "Acme", version: "1.0.0" },
  search: { category: "text", aliases: ["Uppercase"] },
  look: { icon: "text" },
  fields: { text: { type: "text", pin: true } },
  outputs: { result: { type: "text" } },
  run({ fields }) {
    return { result: String(fields.text).toUpperCase() + "!" };
  },
};
```

`export default { … }` works too, and so do TypeScript files: XC Circuit strips the types when it loads them (it doesn't check them; your code editor does, with [`@xc-circuit/sdk`](#typescript)).

## Keys

| Key | Type | Description |
| --- | --- | --- |
| `id` | text | **Required.** Unique id, starting with your plugin's id: `acme.shout`. Never change it once people use the block: projects refer to blocks by id. |
| `kind` | `"event"` · `"action"` · `"value"` | **Required.** See [Events, actions and values](/explanations/blocks-and-wires). |
| `info` | object | What the block is. [Below](#info). |
| `search` | object | How the block picker finds it. [Below](#search). |
| `look` | object | How it looks. [Below](#look). |
| `requires` | object | What it needs to run. [Below](#requires). |
| `fields` | object | Inputs filled in on the block (or connected), by key. See [Fields and types](./fields). |
| `outputs` | object | Values it gives, by key: `{ type, label?, description? }`. |
| `flows` | list of text | Action outputs of an action with more than one way out. [Below](#flows). |
| `pure` | boolean | Value blocks only. By default a value block runs once per run and its outputs are reused; `pure: false` runs it again every time a value is needed (for random numbers, the current time…). |
| `run` | function | What the block does. [Below](#run). |
| `listen` | function | Events only: subscribes to what starts a flow. [Below](#listen). |
| `setup` | function | Runs once when the bot starts, before events subscribe. |
| `define` | function | Runs in the **editor** to change fields and outputs from the block's values and connections. [Below](#define). |
| `migrate` | function | Upgrades saved values when the major version changes. [Below](#migrate). |

The keys inside `info`, `search`, `look` and `requires` may also stand at the top level (older block files do this); a key given in both places is reported.

### info

| Key | Description |
| --- | --- |
| `name` | **Required.** The block's name, in English, in Title Case: *Send Message*. |
| `description` | **Required.** One or two sentences: what it does, for someone who hasn't used it. |
| `author` | **Required.** Who wrote it. |
| `version` | **Required.** `major.minor.patch`. The **major** version is saved with every use of the block (`acme.shout@1`); raise it when old saved values need [`migrate`](#migrate). |
| `docs` | A web address with more help. |
| `deprecated` | `true` or a text saying what to use instead. The block stays usable, but the picker hides it. |

### search

| Key | Description |
| --- | --- |
| `category` | A category id of the build mode, such as `send`, `members`, `text` or `flow` (see [Build modes](./build-modes#categories)). Events go into the category of the blocks they work with. |
| `aliases` | Other English names the picker finds the block by (*Reply* for Send Message). They rank like the name and are never shown. |
| `tags` | More search words, ranked after the name and aliases. |

### look

| Key | Description |
| --- | --- |
| `icon` | An icon name from any plugin's `icons`: `message`, `user`, `clock`, `hash`, `branch`… |
| `width` | The block's width on the canvas, in points. |
| `preview` | A built-in preview drawn on the block. `embed` draws the block's embed as Discord shows it (and makes the block wider). |

### requires

| Key | Description |
| --- | --- |
| `intents` | Discord gateway intents the block needs, such as `GuildMessages` or `GuildMembers`. The bot asks Discord for the intents of the blocks used. |
| `dependencies` | npm packages the block's code needs: `{ "node-fetch": "^3.3.0" }`. They're installed before Run and listed in exports. Load them with [`ctx.require`](./ctx#require). |
| `permissions` | The bot's permissions the block needs, by the platform's flag names: `["ManageRoles"]`, or a list per value of one field: `{ action: { kick: ["KickMembers"], ban: ["BanMembers"] } }`. When the platform answers "Missing Permissions", the error names them. |

Discord's privileged intents (`GuildMembers`, `GuildPresences`, `MessageContent`) have to be turned on for the bot in the developer portal; Bot settings lists the ones the project's blocks need, with a link there.

## run

```js
async run({ fields, ctx, node }) { … }
```

| Argument | Description |
| --- | --- |
| `fields` | Every field's value, with `${…}` already filled in and connected values read. |
| `ctx` | The [run context](./ctx): logging, variables, stored data, the Discord client… |
| `node` | `{ id, workspace }`: which block on which canvas is running. |

What `run` does depends on the kind:

- **Value blocks** return their outputs: `return { result: 42 }`. They run when an action needs one of the outputs.
- **Actions without `flows`** return their outputs (or nothing). The flow continues on their single action output after `run` finishes.
- **Actions with `flows`** call [`ctx.next(flow, outputs)`](./ctx#next) for the way to continue, once or several times (a loop calls it for every item).

`run` may be `async`; write it that way only when it awaits something. The block picker tags a block **Async** when its `run` waits for something (Discord, the web, a timer, stored data): when it uses `await`, `.then(` or a new `Promise`. Waiting for the rest of the flow (`await ctx.next(…)`) doesn't count. Every other action and value block is **Sync**. Searching for *async* or *sync* finds them.

Throw an `Error` to fail the block; the message is shown on the block and in the Console.

## flows

```js
flows: ["true", "false"],
async run({ fields, ctx }) {
  await ctx.next(fields.condition ? "true" : "false");
}
```

Each name becomes an action output. A loop:

```js
flows: ["each", "done"],
outputs: { item: { type: "any" }, index: { type: "number" } },
async run({ fields, ctx }) {
  for (const [index, item] of fields.list.entries()) await ctx.next("each", { item, index });
  await ctx.next("done");
}
```

## listen

Event blocks subscribe in `listen(ctx, emit)`. Each call of `emit(outputs)` starts a new run with those outputs; it returns a promise of the value the run gives back with Return.

```js
listen(ctx, emit) {
  ctx.discord.client.on("guildMemberAdd", (member) => {
    emit({ member, user: member.user, server: member.guild });
  });
},
```

`ctx.fields` holds the event block's own field values. Use [`ctx.runtime.onStart`](./ctx#runtime) for work that should wait until the bot is connected.

## define

`define()` runs **in the editor** (in a sandbox, with a 50 ms limit) whenever the block's values or connections change. It changes the block's shape: extra fields, hidden fields, the type of an output. It can't do anything else: no network, no files.

```js
define({ values, connected, project }) {
  // A For Each over a list of members gives members.
  return { outputs: { item: { type: circuit.types.elementOf(connected.list) } } };
}
```

| Argument | Description |
| --- | --- |
| `values` | The values filled in on the block. |
| `connected` | The type string of what's connected to each input, by field key. |
| `project` | `{ data, settings }`: the project's data, such as `slashCommands`, and Bot › Settings (without secrets), such as `databases`. |

It returns a [DefineResult](./fields#define-results): `add`, `hide`, `set`, `outputs`, `flows`. The global `circuit` has helpers: `circuit.types.elementOf("list<discord.role>")` is `"discord.role"`, `circuit.types.shape("object<{ name: text }>")` is `{ name: "text" }`, and `circuit.object.keysToChoices(shape)` turns a shape into choices.

Top-level `require()` calls return a harmless stand-in in the editor, so a block file that needs a package at run time still loads.

## migrate

```js
info: { …, version: "2.0.0" },
migrate(values, fromVersion) {
  // In version 1, "message" was called "text".
  if (fromVersion === "1") return { ...values, message: values.text };
  return values;
}
```

When a project saved with an older **major** version opens, XC Circuit calls `migrate` with the saved values and the old major version, saves what it returns, and updates the block to the new version. Without `migrate`, the values are kept as they are.

## TypeScript

Block files can be `.ts`. For types in your code editor, use the `@xc-circuit/sdk` package (in the repository's [`sdk/`](https://github.com/XCraftTM/XC-Circuit/tree/main/sdk) folder):

```ts
import { defineBlock } from "@xc-circuit/sdk";

interface Fields { xp: number; perLevel: number }

export default defineBlock<Fields>({
  id: "demo.level_from_xp",
  kind: "value",
  info: { name: "Level From XP", description: "Turns XP into a level.", author: "You", version: "1.0.0" },
  fields: { xp: { type: "number", pin: true }, perLevel: { type: "number", default: 100 } },
  outputs: { level: { type: "number" } },
  run({ fields }) {
    return { level: Math.floor(fields.xp / fields.perLevel) + 1 };
  },
});
```

`defineBlock` only helps the types: it returns the block unchanged.

## Writing good blocks

- **English, plain words.** Names in Title Case, descriptions for people who don't program.
- **One job per block.** Combine blocks on the canvas rather than adding modes to one.
- **Accept ids and objects.** Where a block needs a user, take a user, a member or an id.
- **Fail with a reason.** `throw new Error("Connect a channel or type its id")`, not a stack trace.
- **Keep ids and field keys stable.** Projects store them.
