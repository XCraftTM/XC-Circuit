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
| Remove a wire | Click it and press Delete, or right-click it › Delete wire; or drag it off its input and drop it on empty canvas |
| Undo / Redo | Ctrl+Z / Ctrl+Y (per workspace) |
| Copy / Paste / Duplicate | Ctrl+C / Ctrl+V (at the mouse) / Ctrl+D; works across workspaces and projects |
| Command palette | Ctrl+P or the search box in the title bar: run any command, open a workspace, or jump to a block |
| Title bar | Drag to move, double-click to maximize, drag the window edges to resize; View › Use system title bar switches to the system one |
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
crates/xc-circuit-plugin the crate native Rust plugins are written with
runtime/                 the Node.js runtime that executes block graphs (also inside exports)
plugins/xc.core          flow/text/math/logic/data/debug blocks, icons, themes
plugins/xc.discord       Discord build mode, pin types and blocks
plugins/xc.stats         example native Rust plugin (Project Stats panel)
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

## Native Rust plugins

A plugin can include a compiled Rust library. It adds dockable panels drawn directly with egui, commands (in the Plugins menu and the command palette), checks that show in the Problems panel, and hooks for saves and for what the running bot does. It can read and change the project, select blocks, log to the Console and keep its own small key-value store.

```rust
use xc_circuit_plugin::{egui, Command, Host, Level, PanelInfo, Plugin};

#[derive(Default)]
struct Hello { clicks: u32 }

impl Plugin for Hello {
    fn commands(&self) -> Vec<Command> { vec![Command::new("hello.greet", "Say hello")] }
    fn run_command(&mut self, _id: &str, host: &mut dyn Host) { host.log(Level::Info, "Hello from Rust!"); }
    fn panels(&self) -> Vec<PanelInfo> { vec![PanelInfo::new("hello", "Hello")] }
    fn panel_ui(&mut self, _id: &str, ui: &mut egui::Ui, _host: &mut dyn Host) {
        if ui.button(format!("Clicked {} times", self.clicks)).clicked() { self.clicks += 1; }
    }
}

xc_circuit_plugin::export_plugin!(Hello);
```

The plugin folder has a `plugin.toml` with `[native] library = "my_plugin"` and a Cargo crate with `crate-type = ["cdylib"]`. See `plugins/xc.stats`.

- **Build it with the editor.** A native plugin shares egui with the editor, so it must come from the same build: add the crate to the workspace `members` and build with `cargo plugin <crate>` (an alias for building it together with the editor; add `--release` for a release editor). XC Circuit compares a fingerprint (compiler, crate versions, type ids, debug/release) before loading and refuses a plugin that differs, saying why.
- **Turned off by default.** Plugins › Manage Plugins › Allow native plugins. Plugins outside the app's own `plugins/` folder (for example in a project you downloaded) must also be allowed one by one.
- **Rebuilds reload.** The editor loads a copy of the library and notices when the original changes, so rebuilding while it runs swaps the plugin in. A plugin that panics is switched off until it is rebuilt.
- The editor looks for the library in the plugin folder, its `native/` folder, then next to the editor executable (where `cargo plugin` puts it).

## Status

Done: editor foundation, running and exporting bots, Try Zones, undo/redo, multi-select and clipboard, Embed and Slash Command builders, 49 bundled blocks, plugin hot reload, Stored Data panel, TypeScript blocks, command palette, custom title bar with search, native Rust plugins, and the Windows/Linux build workflow (`.github/workflows/build.yml`, releases on `v*` tags).

Not done yet: sandboxed WebAssembly plugins (the safer plugin tier from the spec), sharing plugins from GitHub inside the editor, the web dashboard blocks, and converting a project to a TypeScript bot.
