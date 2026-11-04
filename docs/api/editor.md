# Editor plugins (xc)

An editor script adds panels, commands and checks to XC™ Circuit, and reacts to what happens in the app. Name it in `plugin.toml`:

```toml
[editor]
script = "editor.js"   # or editor.ts
```

The script runs once when the plugin loads, and again whenever one of its files is saved. Its job at that point is to **register** things with `xc.command`, `xc.panel`, `xc.check` and `xc.on`. Everything else (reading the project, logging…) happens inside the functions it registers.

```js
xc.command({ id: "hello.greet", title: "Say hello", run() { xc.log("Hello from", xc.project()?.name) } });
```

## The sandbox

Editor scripts run in their own QuickJS sandbox, inside the editor:

- They reach the app only through `xc`. There is no `fetch`, no file system, no `process`, no timers.
- Each call (a command, a render, a check) may take **100 ms** (render, checks, bot events) or **1 s** (commands, clicks, saves). A script that takes longer is stopped and the error says so.
- Each plugin may use up to **32 MB** of memory.
- `xc` can't be used while the script loads (only in the functions it registers).

Errors name the file and line. A failing command or handler is logged to the Console; a failing check shows in the Problems panel; a failing panel shows its error in the panel. The plugin keeps running either way.

Modern JavaScript works (`?.`, `??`, classes, destructuring, `async` functions, though nothing waits for their promises). TypeScript files (`.ts`) have their types stripped.

### Modules

`require("./lib/util.js")` loads another file of the plugin (relative to the file asking; `.js` is added if missing; `.ts` and `.json` work too). Only files inside the plugin folder can be loaded. Each file is loaded once and shared, like in Node.js.

`console.log`, `console.warn` and `console.error` write to the Console, like `xc.log`.

## Registering

### xc.command

```js
xc.command({ id, title, description?, run() })
```

Adds a command to the **Plugins** menu and the **command palette** (Ctrl+P). `id` must be unique across plugins; start it with your plugin's id. `run()` is called when the command is used.

### xc.panel

```js
xc.panel({ id, title, render(ui) })
```

Adds a dockable panel, opened from the Plugins menu or the command palette. `render(ui)` describes its contents with [`ui`](./panels); XC Circuit calls it whenever the panel is drawn (at most four times a second while nothing happens, right away after a click), so it always shows the current state. See [Panels (ui)](./panels).

### xc.check

```js
xc.check(() => problems)
```

Adds a check. XC Circuit runs checks about once a second while a project is open, and shows what they return in the **Problems** panel. Return an array (or a single item, or nothing) of:

- a text: `"The bot has no welcome message"`, or
- `{ message, workspace?, block? }`: with `workspace` and `block`, clicking the problem selects the block.

```js
xc.check(() =>
  xc.blocks()
    .filter((b) => b.type === "core.log" && !b.values.message)
    .map((b) => ({ message: "Log has no message", workspace: b.workspace, block: b.id })),
);
```

### xc.on

```js
xc.on("save", () => { … })
xc.on("botEvent", (event) => { … })
```

| Event | When | Argument |
| --- | --- | --- |
| `save` | after the project was saved | — |
| `botEvent` | when the bot started from the editor does something | the event, below |

Bot events:

| `type` | Other keys |
| --- | --- |
| `"started"` | — |
| `"stopped"` | — |
| `"log"` | `level` (`info`, `success`, `warn`, `error`), `text` |
| `"block"` | `workspace`, `block` (its id), `phase` (`enter`, `exit`, `error`), `ms` (on exit), `error` (on error) |

Block events come for every block that runs, so keep the handler quick.

## The project

| Function | Returns |
| --- | --- |
| `xc.project()` | `{ name, root }` of the open project, or `null` |
| `xc.workspaces()` | `[{ key, name }]`, every workspace |
| `xc.workspace(key)` | the workspace as saved: `{ name, folder?, nodes, links, zones? }`, or `null`. See [Project format](./project-format#workspaces). |
| `xc.blocks()` | every block of every workspace: `[{ workspace, id, type, block, at, values, pins?, zone? }]`; `type` is the block id without its version (`core.log`) |
| `xc.activeWorkspace()` | the key of the workspace open in the canvas, or `null` |
| `xc.selection()` | the ids of the selected blocks in the active workspace |
| `xc.blockName(type)` | the display name of a block type: `xc.blockName("core.log")` is `"Log"` |
| `xc.botRunning()` | whether the bot is running |

### Changing the project

| Function | Does |
| --- | --- |
| `xc.setWorkspace(key, workspace)` | Replaces a workspace with a changed copy (from `xc.workspace(key)`). Can be undone with Ctrl+Z, and marks the project unsaved. |
| `xc.data.get(key)` | A value of the project's `data` (saved in `circuit.json`), or `null`. |
| `xc.data.set(key, value)` | Sets it (anything JSON can hold). Marks the project unsaved. |

```js
const ws = xc.workspace("welcome");
ws.nodes["3"].values.message = "Welcome, ${user}!";
xc.setWorkspace("welcome", ws);
```

Keys in `data` are shared by all plugins and the editor (`slashCommands` is the Slash Command Builder's), so use keys of your own, such as `"acme.notes"`.

## The app

| Function | Does |
| --- | --- |
| `xc.log(...parts)` | Writes to the Console. Objects are written as JSON. |
| `xc.success(...)`, `xc.warn(...)`, `xc.error(...)` | The same, as a success, a warning or an error. |
| `xc.reveal(workspace, block)` | Opens the workspace, selects the block and centres the canvas on it. |
| `xc.run(id)` | Runs a command after the current handler: a plugin's command, or `bot.run`, `bot.stop`, `file.save`. |
| `xc.storage.get(key)` | A value from this plugin's own store, or `null`. |
| `xc.storage.set(key, value)` | Stores a value (anything JSON can hold). Kept between sessions on this computer, not in the project. |
| `xc.plugin.name` | The plugin's name from `plugin.toml`. |

## State

Variables at the top of the script live as long as the plugin: use them for a panel's state. They start over when the script reloads (when you save it). Use `xc.storage` for what should outlast that, and `xc.data` for what belongs to the project.

```js
let filter = "";

xc.panel({
  id: "find",
  title: "Find",
  render(ui) {
    ui.input(filter, (text) => (filter = text), { placeholder: "Block name" });
    for (const b of xc.blocks().filter((b) => xc.blockName(b.type).toLowerCase().includes(filter.toLowerCase()))) {
      ui.link(`${xc.blockName(b.type)} (#${b.id})`, () => xc.reveal(b.workspace, b.id));
    }
  },
});
```

## A complete example

DemoBot's `plugins/demo.custom/editor.js` has a panel with notes, TODOs, workspaces and what the bot did, a command and a check. Open DemoBot from Help › Open Example and see it under Plugins.
