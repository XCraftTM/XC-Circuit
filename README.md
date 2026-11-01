# XC™ Circuit

Build bots by connecting blocks on a canvas. Native Rust editor (egui), plugin-driven: blocks are plain JavaScript files, and the Discord mode, its blocks, pin types and themes all come from plugins in `plugins/`.

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
| Delete | Select a block, press Delete (or right-click › Delete) |
| Save | Ctrl+S (restarts a running bot) |
| Panels | Drag tabs to dock anywhere; reopen closed ones from View › Panels |
| Try Zone | Right-click a block › Wrap in Try Zone; drop blocks in or out; right-click the zone's title for retries and timeout |
| Bot token | Bot › Settings (kept in Windows Credential Manager / Linux Secret Service) |
| Run / Stop | F5 / Shift+F5, or Run in the title bar. Running blocks glow; failures show on the block |
| Export | Bot › Export… writes a folder that runs with `node index.js` |

Running needs Node.js 22 or newer. The first run installs the mode's npm packages (discord.js) once, into `%APPDATA%\XC Circuit\packages` or `~/.local/share/xc-circuit/packages`.

## Tests

```
cargo test
node runtime/test/smoke.js
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
examples/DemoBot         example project
```

Extra plugin folders can be listed in `XC_CIRCUIT_PLUGINS`. Installed plugins live in `%APPDATA%\XC Circuit\plugins` (Windows) or `~/.local/share/xc-circuit/plugins` (Linux); a project's own `plugins/` folder is loaded too.

## Status

Milestone 1 (editor foundation) and milestone 2 (running, Try Zones, export) are done.

Next: undo/redo and multi-select, field-type editors (Embed and Slash Command builders, slash command options), bundling npm packages into `index.js`, TypeScript blocks, hot reload of plugins, the custom title bar, and the Windows/Linux build workflow.
