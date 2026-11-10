# Editor plugins (xc)

::: info In short
An `editor.js` in your plugin, named in `plugin.toml` under `[editor] script = "editor.js"`. It uses the global `xc` to add commands, panels, checks and overlays to XC™ Circuit, and to read and change the project. It reloads when you save it.
:::

```js
xc.command({ id: "hello.greet", title: "Say hello", run() { xc.log("Hello from", xc.project()?.name) } });
```

Types for your code editor: `/// <reference types="@xc-circuit/sdk/editor" />` (from the repository's `sdk/` folder).

## Quick reference

### Add things to the app

| | |
| --- | --- |
| [`xc.command({ id, title, run })`](#xc-command) | a command in the Plugins menu and the command palette |
| [`xc.panel({ id, title, render })`](#xc-panel) | a dockable panel |
| [`xc.check(fn)`](#xc-check) | problems for the Problems panel |
| [`xc.overlay({ id, width, height, render, onClick })`](#xc-overlay) | something floating over the editor |
| [`xc.on(event, fn)`](#xc-on) | react to `"save"`, `"botEvent"`, `"selection"` |
| [`xc.every(ms, fn)` · `xc.after(ms, fn)` · `xc.cancel(id)`](#timers) | timers |
| [`xc.playSound(file, { volume })`](#sounds) | play a sound from the plugin |
| [`xc.tool({ name, description, input, run })`](#xc-tool) | something AI agents can do through the MCP server |

### Read the project

| | |
| --- | --- |
| `xc.project()` | `{ name, root }` or `null` |
| `xc.workspaces()` | `[{ key, name }]` |
| `xc.workspace(key)` | a workspace as saved: `{ name, nodes, links, zones, notes }` |
| `xc.blocks()` | every block: `[{ workspace, id, type, values, … }]` |
| `xc.activeWorkspace()` · `xc.selection()` | what's open and selected |
| `xc.blockName(type)` | `"core.log"` → `"Log"` |
| `xc.data.get(key)` | project data (the project's `data`) |
| `xc.storage.get(key)` | the plugin's own store (on this computer) |
| `xc.botRunning()` · `xc.screen()` | is the bot running · window size |
| `xc.blockTypes(query)` · `xc.blockType(type)` | installed block types · one type's fields, outputs and flows |
| `xc.blockSource(type)` · `xc.plugins()` | a block type's code: `{ type, plugin, file, version, source }` · the plugins, turned on or off, with their blocks |
| `xc.appSettings()` · `xc.botSettings()` | the app's settings (with the shortcuts) · Bot › Settings: `{ mode, pages, settings: [{ key, label, type, page, value, … }] }` (secrets only say whether one is saved) |
| `xc.server()` | the bot's server (Bot › Server…): kind, where, `runTarget` (`local` or `server`), `canRunThere`, `savedHere` (false for a server that came with the project's files and the user hasn't saved on this computer: nothing is sent to it), `state`, CPU and memory, `lastUpload`, the server console's last lines; never a password or key |
| `xc.blockPins(workspace, id)` | a block as it is now: its values and pins, with types and wires |
| `xc.problems()` · `xc.console(lines)` | what the Problems panel and the Console show |
| `xc.t(text)` | text in the app's language, from the plugins' `locales/*.toml` |

### Change things

| | |
| --- | --- |
| `xc.addBlock(workspace, type, { x, y, values })` | place a block, returns its id |
| `xc.setWorkspace(key, workspace)` | replace a workspace |
| `xc.connect(workspace, "3.next", "4.in")` | draw a wire, checked like one drawn on the canvas |
| `xc.addWorkspace(name, { folder })` | make a workspace, returns its key |
| `xc.copyText(text)` | put text on the clipboard |
| `xc.fieldValue(type, field, value)` | a value as the block keeps it: embeds, layouts and canvases as the builders save them |
| `xc.openBuilder(workspace, id, field)` | open the Embed, Layout or Canvas Builder on a block's field |
| `xc.blockRects(workspace)` | where its blocks are and how big, as drawn: `{ id: [x, y, width, height] }` |
| `xc.select(workspace, ids)` · `xc.reveal(workspace, id, zoom?)` | select · go to a block (at `zoom` percent, 25 to 200, when given) |
| `xc.data.set(key, value)` · `xc.storage.set(key, value)` | save data (not the key `hosting`: the bot's server is the user's to set, in Bot › Server…) |
| `xc.log(…)` · `xc.success(…)` · `xc.warn(…)` · `xc.error(…)` | write to the Console |
| `xc.setBotSettings({ key: value })` | change Bot › Settings (not secrets), checked against each setting's type and choices; returns the keys changed |
| `xc.run(id)` | run a command: a plugin's, or `"bot.run"`, `"bot.stop"`, `"file.save"`, `"server.upload"`, `"server.start"`, `"server.restart"`, `"server.stop"`, `"server.kill"`, `"run.local"`, `"run.server"`. Starting the bot or acting on its server (`bot.run`, `server.*`, `run.*`) only works from a command or a click (a button, a link, an overlay's `onClick`) or an agent's tool, not from a timer, a check or `render`: it throws there |
| `xc.moveOverlay(id, x, y)` · `xc.overlayPosition(id)` | move an overlay |

Every change can be undone with Ctrl+Z, like your own edits.

## Adding things

### xc.command

```js
xc.command({ id: "acme.tidy", title: "Tidy up", description: "Shown on hover", run() { … } });
```

The `id` must be unique: start it with your plugin's id.

### xc.panel

```js
xc.panel({ id: "stats", title: "Stats", render(ui) { ui.heading("Hello") } });
```

Opened from the Plugins menu. `render(ui)` describes the panel with [`ui`](./panels); it runs whenever the panel is drawn, so it always shows the current project.

### xc.check

```js
xc.check(() =>
  xc.blocks()
    .filter((b) => b.type === "core.log" && !b.values.message)
    .map((b) => ({ message: "Log has no message", workspace: b.workspace, block: b.id })),
);
```

Runs about once a second. Return problems: a text, or `{ message, workspace, block }` (then clicking the problem selects the block).

### xc.overlay

```js
xc.overlay({
  id: "pet",
  width: 96, height: 96,
  x: -28, y: -60,            // where it starts; negative: from the right or bottom edge
  render(ui) { ui.image("art/pet.svg", { width: 96 }) },
  onClick() { xc.playSound("sounds/boop.wav") },
});
```

Floats over the whole editor. People can drag it (`draggable: false` stops that). `frame: true` puts it on a card. Drawn again after a click, a timer or an event, and at least once a second. Example: [Desk Pet](https://github.com/XCraftTM/XC-Circuit/tree/main/examples/plugins/desk-pet).

### xc.on

| Event | When | `fn` gets |
| --- | --- | --- |
| `"save"` | the project was saved | — |
| `"selection"` | other blocks were selected | `{ workspace, blocks }` |
| `"botEvent"` | the bot (run from the editor) did something | `{ type: "started" }`, `{ type: "stopped" }`, `{ type: "log", level, text }`, `{ type: "block", workspace, block, phase, ms, error }` |

Block events come for every block that runs: keep that handler quick.

### Timers

```js
const id = xc.every(500, () => { … });   // every half second (at least 50 ms)
xc.after(2000, () => xc.cancel(id));      // once, two seconds later
```

They also work at the top of the script. A timer that throws is stopped and the error logged.

### Sounds

```js
xc.playSound("sounds/ding.wav", { volume: 0.5 });
```

WAV, MP3 or Ogg Vorbis from the plugin's folder. Nothing plays (and nothing fails) without an audio device.

### xc.tool

Something AI agents such as Claude can do in XC Circuit, through its [MCP server](/explanations/ai-agents). `input` is a JSON Schema for the arguments; `run(args)` returns the answer: text, or any value (the agent gets it as JSON). Throwing an error tells the agent what went wrong. A tool may take 5 seconds.

```js
xc.tool({
  name: "count_blocks",
  description: "How many blocks each workspace has.",
  input: { type: "object", properties: {} },
  run() {
    return xc.workspaces().map((w) => ({ workspace: w.name, blocks: Object.keys(xc.workspace(w.key).nodes).length }));
  },
});
```

The tools of every plugin that is on are offered together; the bundled AI agents plugin has the usual ones. `xc.agents` (`clients()`, `install(id)`, `remove(id)`, `snippet()`, `link()`) is what its panel uses to add the server to AI clients.

## Good to know

**Sandbox.** Scripts run in their own JavaScript engine (QuickJS) and only reach the app through `xc`: no files, no network, no programs. That's why they load without asking you first.

**Time limits.** A render, check or event may take 100 ms; a command, click or save 1 s. Longer, and it's stopped with an error. Memory: 32 MB per plugin.

**Errors** name the file and line: in the Console for commands and events, in the Problems panel for checks, in the panel itself for panels.

**Modules.** `require("./lib/util.js")` loads another file of the plugin (`.js`, `.ts`, `.json`). Only the plugin's own files: not one a link in its folder points to elsewhere, and no name with a colon in it.

**State.** Variables at the top of the script live until the script reloads. Use `xc.storage` for what should last, `xc.data` for what belongs to the project. Keys in `xc.data` are shared, so prefix yours: `"acme.notes"`.

**Not while loading.** The top of the script registers things (and may start timers). The project is only reachable inside the functions you register.

Bigger example: DemoBot's `plugins/demo.custom/editor.js` (panel, command, check).
