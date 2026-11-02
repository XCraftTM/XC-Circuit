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
| Add a block | Add block in the canvas toolbar (bottom left), right-click empty canvas, or press Space |
| Connect | Drag from a pin to another pin |
| Add a connected block | Drag a wire into empty space; the picker shows only compatible blocks |
| Move a block | Drag its title bar |
| Pan / zoom | Drag empty canvas or middle mouse / mouse wheel |
| Templates | File › New Project starts empty or from a working bot: Welcome & auto role, Moderation, Levels & XP, Tickets, Role menu (in `examples/templates/`) |
| Merge text | Type `{name}` in any text field (Send Message, Log, Text …): an input named `{name}` appears below it to connect a value to. Left unconnected, the name is filled from the event, variables or an earlier block's output |
| Values from earlier blocks | Type `{` in any text (on a block, in the Inspector or a builder), right-click it, or click the `{}` button in its corner for the values the block can use: the event's outputs (`{user}`), the outputs of the blocks before it on its action path (`{#4.message}`, block #4's Message), run variables, and their properties (`{user.username}`, `{#4.message.url}`). The Embed and Layout Builders list them too: click one to put it where the text cursor is. The list comes from following the Action wires back from the block |
| One path per action | An action output leads to one block: connecting it to another block replaces the old wire. Paths can join, and branch only through blocks made for it (If, Switch, For Each). Older projects with split paths show them in Problems |
| Yes/no inputs | Switches like Private reply can be connected too: click their pin toggle in the Inspector |
| Long text | Text fields on blocks are boxes of a few lines; drag a box's corner down to make it taller or shorter and sideways to make the whole block wider or narrower (the block saves both in the project), double-click the corner to reset them |
| Colors | Click a color swatch for the picker: square and hue bar, hex and RGB, Discord colors and your recent ones |
| Field as pin | Click a field's faint pin, or "pin" in the Inspector |
| Select several | Shift-drag a box on empty canvas, or Ctrl+click blocks; Ctrl+A selects all |
| Block actions | Select a block: Duplicate, Try Zone, Source and Delete are buttons above it (Delete key works too) |
| Remove a wire | Click it, then its Delete wire button (or press Delete); or drag it off its input and drop it on empty canvas |
| Zoom | The canvas toolbar: − / + , the percentage goes back to 100%, the corners button shows every block; or the mouse wheel |
| Undo / Redo | Ctrl+Z / Ctrl+Y (per workspace) |
| Copy / Paste / Duplicate | Ctrl+C / Ctrl+V (at the mouse) / Ctrl+D; works across workspaces and projects |
| First start | The welcome page sets up Node.js (finds it, or installs it: winget or the official installer on Windows, a private copy on Linux), a theme and an accent; Help › Welcome and setup… opens it again |
| Tour | Help › Take the tour: a guided walk through adding, wiring and running blocks on a practice project; skip any time, arrow keys move between steps |
| Backups | Every save keeps a snapshot outside the project (newest 20, plus one a day for 14 days; Settings › Backups). File › Restore from backup… compares them with now and restores the project or one workspace. Unsaved changes are copied every 30 s and offered back after a crash |
| Bot data backups | A bot backs up its saved data (store.json) when it starts and every 6 hours, keeping the newest 10 in data/backups. If the data is empty or damaged it asks: restore a backup or start empty (a dialog in XC Circuit, a question in the terminal) |
| Settings | Ctrl+, or the gear at the bottom of the activity bar: theme, accent color (changes live), touch mode, interface size, canvas, window, bot, plugins, shortcuts |
| Language | English and German come with the app: pick one on the first start page or in Settings › Appearance; it switches right away, block names included |
| Touch mode | Settings › Appearance (or the first start's Look step): bigger buttons, rows and pins with more room between them, for touch screens and pens |
| Command palette | Ctrl+P or the search box in the title bar: run any command, open a workspace, or jump to a block |
| Editor tabs | The Embed Builder, Layout Builder, Slash Commands, Bot settings, Export and Plugins open as tabs in their own floating window; drag a tab onto any side or next to other tabs to dock it, or out again. Closing the tab cancels it |
| Title bar | Drag to move, double-click to maximize, drag the window edges to resize; View › Use system title bar switches to the system one |
| Save | Ctrl+S (restarts a running bot) |
| Panels | Drag tabs to dock anywhere; reopen closed ones from View › Panels |
| Workspaces | The Workspaces panel lists them in folders: + and the folder button on the project row add them, drag a workspace onto a folder to move it, double-click a name to rename, the ⋯ button (or right-click) for more |
| Activity bar | The icons on the far left open Workspaces, Stored Data, Plugins and Bot Settings (View › Show activity bar hides it) |
| Try Zone | Select a block › Try Zone; drop blocks in or out; the gear on the zone's title sets retries and timeout |
| Embeds | Select a Build Embed block › Open Embed Builder; the block shows a live preview. Every part of an embed is there: author with link and icon, title link, fields, images, footer, and a timestamp (when it's sent, or a set time) |
| Components V2 | A Build Layout block designs a whole message from Discord's components: containers with an accent color, text (markdown), sections with a picture or button next to the text, media galleries, files, separators, rows of buttons (every style, emoji, link and premium) and select menus (options, users, roles, channels). Edit layout… opens the Layout Builder with a Discord preview; set Send Message's Format to Layout and connect the block. Pictures and files given as file paths are attached when sending |
| JSON | The Embed Builder, Layout Builder and Slash Commands each switch between Builder and JSON. The JSON is Discord's own: edit it or paste JSON from the Discord docs, discord.js `toJSON()` or a message designer, and it applies as soon as it is valid (errors show the line and column) |
| Slash commands | Bot › Slash Commands…: options with choices (a name and a value each), length and number limits, channel kinds and autocomplete; subcommands and groups; translations of every name and description into Discord's languages; the permissions people need (any number), where the command works (servers, DMs with the bot, other DMs), how the bot is installed (server or account) and age-restricted. Registered when the bot starts |
| Stored data | View › Panels › Stored Data shows what `ctx.store` saved while running |
| Bot token | Bot › Settings (kept in Windows Credential Manager / Linux Secret Service). The eye button shows what you typed, Copy copies the saved token without showing it, and values are checked as you type (a build mode's settings can set `pattern`, a regular expression, and `patternHint`; `sensitive = true` hides a text setting behind the eye button) |
| Run / Stop | F5 / Shift+F5, or Run in the title bar. Running blocks glow; failures show on the block |
| Export | Bot › Export… writes a folder that runs with `node index.js` |

Running needs Node.js 22 or newer; the first start page (or Settings › Bot › Node.js) finds or installs it. The first run installs the mode's npm packages (discord.js) once, into `%APPDATA%\XC Circuit\packages` or `~/.local/share/xc-circuit/packages`.

## Tests

```
cargo test
node runtime/test/smoke.js
node runtime/test/library.js
node runtime/test/template.js
node runtime/test/helpers.js
node runtime/test/discord_blocks.js
node runtime/test/store.js
node runtime/test/backups.js
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

## Translations

A language is a plugin file, `locales/<code>.toml`, listed under `contributes.locales` in the plugin's `plugin.toml`. Settings › Appearance › Create a translation… writes one with every text to fill in (as a plugin in your plugins folder) and opens it; what you save shows right away.

```toml
[meta]
code = "de"
name = "Deutsch"          # the language's own name, shown in the picker
english_name = "German"

[strings]                 # interface texts, by their English text
"Save and close" = "Speichern und schließen"
"{count} to fix" = "{count} zu beheben"   # keep {placeholders}

[labels]                  # block field, output and choice labels
"Message" = "Nachricht"

[blocks."core.math"]      # a block's name and description
name = "Mathe"
description = "Rechnet mit zwei Zahlen."
```

Empty entries show in English, and several plugins can add to the same language (each Build Mode translates its own blocks). German is in `plugins/xc.core/locales/de.toml` and `plugins/xc.discord/locales/de.toml`.

In the editor's code, interface texts are wrapped in `t!("…")` (or `tf!("… {name} …", name = value)`, or `n("…")` in constants). `src/ui_strings.rs` lists them all for templates; a test checks it is current (run the tests with `XC_UPDATE_STRINGS=1` to refresh it) and that German translates every text.

## Status

Done: editor foundation, running and exporting bots, Try Zones, undo/redo, multi-select and clipboard, Embed and Slash Command builders, 138 bundled blocks (with Run Code and Use Method for anything else), plugin hot reload, Stored Data panel, TypeScript blocks, command palette, custom title bar with search, native Rust plugins, and the Windows/Linux build workflow (`.github/workflows/build.yml`, releases on `v*` tags).

The plan for the remaining Discord blocks is in [docs/discord-blocks-plan.md](docs/discord-blocks-plan.md).

Not done yet: sandboxed WebAssembly plugins (the safer plugin tier from the spec), sharing plugins from GitHub inside the editor, the web dashboard blocks, and converting a project to a TypeScript bot.
