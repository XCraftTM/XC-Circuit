# Make a plugin

::: info In short
A plugin is a folder with a `plugin.toml`. Put it in a project's `plugins/` folder (or your own plugins folder), save a file, and XC™ Circuit loads it right away. Blocks run in the bot; an `editor.js` adds panels, commands and checks to the editor. Errors show in the Console and the Problems panel.
:::

This guide takes you from an empty folder to a plugin you can share. The [Reference](/api/) lists every key and function; the [Recipes](./recipes) show complete examples for common tasks; [Share and update](./share) covers packaging and versions.

## What can a plugin do?

| I want to… | Make | Runs in | Start with |
| --- | --- | --- | --- |
| add a **block** to the picker | a block file in `blocks/` | the bot | [Your first block](/tutorials/first-block) |
| add a **panel, command or check** to the editor | an `editor.js` | the editor (sandboxed) | [Your first editor plugin](/tutorials/first-editor-plugin) |
| make something **float over the editor** | an overlay in `editor.js` | the editor | [Desk Pet](https://github.com/XCraftTM/XC-Circuit/tree/main/examples/plugins/desk-pet) |
| change how the app **looks** | a theme | the editor | [Themes](/api/themes) |
| **translate** the app | a locale file | the editor | [Translate the app](/tutorials/translate) |
| offer **starter bots** in New Project | a template | the editor | [Templates](/api/templates) |
| add **settings** to Bot › Settings | a settings file | the editor and the bot | [Plugin settings](/api/build-modes#plugin-settings) |
| support another **chat app** | a build mode, runtime library and pin types | the bot | [Build modes](/api/build-modes) |

One plugin can do several of these: DemoBot's own plugin has blocks and an editor script.

## The folder

```
acme.tools/
├─ plugin.toml        what it is and what it adds
├─ blocks/
│  └─ shout.js        a block
├─ editor.js          panels, commands, checks
└─ icons/
   └─ acme.svg
```

```toml
id          = "acme.tools"
name        = "Acme Tools"
version     = "1.0.0"
author      = "Acme"
description = "A block and a panel for Acme bots."

[engines]
circuit = "^1.0"

[contributes]
blocks = ["blocks/*.js"]
icons  = ["icons/*.svg"]

[editor]
script = "editor.js"
```

Pick an `id` that's yours (`yourname.something`, lowercase) and start every block id and command id with it: two plugins can't use the same ids. Every key is in [plugin.toml](/api/plugin-toml).

## Where to keep it while you work

| Folder | Loaded | Good for |
| --- | --- | --- |
| the project's `plugins/` | only with that project | a plugin for one bot: it travels with the project (and its exports) |
| your plugins folder (Plugins › Open folder) | in every project | a plugin you use everywhere |
| a folder in `XC_CIRCUIT_PLUGINS` | in every project | developing a plugin in its own Git repository |

`XC_CIRCUIT_PLUGINS` is an environment variable with one or more folders (separated by `;` on Windows, `:` on Linux), each holding plugin folders. Start XC Circuit from a terminal that has it set:

```powershell
$env:XC_CIRCUIT_PLUGINS = "C:\code\my-plugins"; & "C:\Program Files\XC Circuit\xc-circuit.exe"
```

## The loop: save, look, fix

XC Circuit watches plugin files. **Save a file and it loads again**: a block file, the editor script, a theme, a translation. You don't restart anything:

- **Blocks** appear in the picker (search for their name). Blocks already on the canvas take the new shape; a running bot uses the new code after a restart (Save restarts it, if *Restart on save* is on).
- **Editor scripts** start again: their panels are drawn again, their commands are in the Plugins menu at once. Variables at the top of the script start over; what's in `xc.storage` and `xc.data` stays.
- **Themes and translations** apply right away.

The Plugins view in the side bar lists every plugin and whether its script runs. **Reload** there loads every plugin again, for a file the watcher missed.

## When something's wrong

| Where | What you see |
| --- | --- |
| **Problems** panel | a `plugin.toml` or block file that couldn't be read, a missing dependency, a block id used twice, problems your `xc.check` returns |
| **Console** | errors from commands, timers and events (with the file and line), what your blocks log with `ctx.log`, and a block that fails while the bot runs |
| the **panel** itself | an error in its `render`, above what it showed last |
| the **block** on the canvas | a block that failed turns red (the Console says why) |
| Plugins view | *Script · error*, with the reason |

An editor script that crashes is turned off until you save one of its files. One that takes too long (100 ms to draw, 1 s for a command) is stopped with an error saying so.

**Logging.** In blocks: `ctx.log.info("…", value)`. In editor scripts: `xc.log("…", value)`. Both go to the Console; objects are shown readably.

## TypeScript and your code editor

Blocks and editor scripts can be TypeScript (`.ts`): XC Circuit removes the types when it loads them. For completion and checking in your code editor, use the types in the repository's [`sdk/`](https://github.com/XCraftTM/XC-Circuit/tree/main/sdk) folder:

```ts
// a block
import { defineBlock } from "@xc-circuit/sdk";
export default defineBlock({ id: "acme.shout", kind: "value", /* … */ });
```

```js
// editor.js
/// <reference types="@xc-circuit/sdk/editor" />
xc.command({ id: "acme.hello", title: "Hello", run() { xc.log("Hi") } });
```

Split a bigger editor script into files with `require("./lib/util.js")` (`.js`, `.ts` or `.json`, only the plugin's own files). A block is one file.

## What's allowed where

| | Editor script (`editor.js`) | Block (in the bot) |
| --- | --- | --- |
| Files, network, programs | no | yes (it's a Node.js program) |
| npm packages | no | yes: `requires.dependencies` and [`ctx.require`](/api/ctx#require) |
| The project | read, and change with undo | only its own values and what's wired in |
| Time | 100 ms to draw, 1 s for a command | as long as it needs |
| Asks before loading | no | no, so only use block plugins you trust |

Because editor scripts can't reach files or the network, they start without asking, even in a project you downloaded. Block code runs with the bot's full access, like any npm package: see [Tokens and safety](/explanations/safety).

## Next

- [Recipes](./recipes): complete examples to copy.
- [Share and update](./share): versions, zips, updates.
- [Reference](/api/): every key and function.
