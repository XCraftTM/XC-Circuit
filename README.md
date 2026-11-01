# XC™ Circuit

Build bots by connecting blocks on a canvas. Native Rust editor (egui), plugin-driven: blocks are plain JavaScript or TypeScript files, and the Discord mode, its blocks, pin types and themes all come from plugins in `plugins/`.

## Run

```
cargo run -p circuit-editor
```

The first start opens `examples/DemoBot`. Windows and Linux are supported.

## Using the editor

| Action | How |
| --- | --- |
| Add a block | Right-click empty canvas, or press Space |
| Connect | Drag from a pin to another pin |
| Add a connected block | Drag a wire into empty space; the picker shows only compatible blocks |
| Move a block | Drag its title bar |
| Pan / zoom | Drag empty canvas or middle mouse / mouse wheel |
| Field as pin | Click a field's faint pin, or "pin" in the Inspector |
| Select several | Shift-drag a box on empty canvas, or Ctrl+click blocks; Ctrl+A selects all |
| Delete | Select blocks, press Delete (or right-click › Delete) |
| Undo / Redo | Ctrl+Z / Ctrl+Y (per workspace) |
| Copy / Paste / Duplicate | Ctrl+C / Ctrl+V (at the mouse) / Ctrl+D; works across workspaces and projects |
| Command palette | Ctrl+P: run any command, open a workspace, or jump to a block |
| Save | Ctrl+S (restarts a running bot) |
| Panels | Drag tabs to dock anywhere; reopen closed ones from View › Panels |
| Try Zone | Right-click a block › Wrap in Try Zone; drop blocks in or out; right-click the zone's title for retries and timeout |
| Embeds | Select a Build Embed block › Open Embed Builder; the block shows a live preview |
| Slash commands | Bot › Slash Commands… (options, choices, permissions); registered when the bot starts |
| Stored data | View › Panels › Stored Data shows what `ctx.store` saved while running |
| Bot token | Bot › Settings (kept in Windows Credential Manager / Linux Secret Service) |
| Run / Stop | F5 / Shift+F5, or Run in the title bar. Running blocks glow; failures show on the block |
| Export | Bot › Export… writes a folder that runs with `node index.js` |

Running needs Node.js 22 or newer. The first run installs the mode's npm packages (discord.js) once, into `%APPDATA%\XC Circuit\packages` or `~/.local/share/xc-circuit/packages`.

## Tests

```
cargo test
node runtime/test/smoke.js
node runtime/test/library.js
```

## Layout

```
crates/circuit-core      graph model, pin types, block definitions, project format
crates/circuit-plugins   plugin.toml, contributions, QuickJS sandbox for block files
crates/circuit-runner    bundles, npm packages, the bot process, debug protocol, secrets, export
crates/circuit-editor    the egui app (bin: xc-circuit)
runtime/                 the Node.js runtime that executes block graphs (also inside exports)
plugins/xc.core          flow/text/math/logic/data/debug blocks, icons, themes
plugins/xc.discord       Discord build mode, pin types and blocks
examples/DemoBot         example project (with a TypeScript block in its own plugins/ folder)
sdk/                     @xc-circuit/sdk: types for writing blocks
```

Extra plugin folders can be listed in `XC_CIRCUIT_PLUGINS`. Installed plugins live in `%APPDATA%\XC Circuit\plugins` (Windows) or `~/.local/share/xc-circuit/plugins` (Linux); a project's own `plugins/` folder is loaded too.

## Writing blocks

A block is one `.js`, `.cjs` or `.ts` file in a plugin's `blocks/` folder. Plugin files are watched: saving one reloads it in the editor. TypeScript is converted when the plugin loads; for types, add `@xc-circuit/sdk`:

```ts
import { defineBlock } from "@xc-circuit/sdk";

export default defineBlock({
  id: "demo.level_from_xp", name: "Level from XP", kind: "value",
  description: "Turns XP into a level.", author: "You", version: "1.0.0",
  fields: { xp: { type: "number" }, perLevel: { type: "number", default: 100 } },
  outputs: { level: { type: "number" }, toNext: { type: "number" } },
  run({ fields }) {
    // A value block returns its outputs.
    return { level: Math.floor(fields.xp / fields.perLevel) + 1, toNext: fields.perLevel - (fields.xp % fields.perLevel) };
  },
});
```

See `examples/DemoBot/plugins/demo.custom` and `sdk/index.d.ts`.

## Status

Done: editor foundation, running and exporting bots, Try Zones, undo/redo, multi-select and clipboard, Embed and Slash Command builders, 49 bundled blocks, plugin hot reload, Stored Data panel, TypeScript blocks, command palette, and the Windows/Linux build workflow (`.github/workflows/build.yml`, releases on `v*` tags).

Not done yet: custom window title bar, native Rust plugins, sharing plugins from GitHub inside the editor, the web dashboard blocks, and converting a project to a TypeScript bot.
