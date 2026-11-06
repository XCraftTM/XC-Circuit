# Contributing

Thanks for wanting to help! XC™ Circuit welcomes **bug reports, ideas, translations, blocks, fixes and docs**.

## Reporting and ideas

- **Bugs:** [open a bug report](https://github.com/XCraftTM/XC-Circuit/issues/new?template=bug_report.yml), or use Help › Report a Bug in the app (it fills in your version).
- **Ideas:** [suggest a feature](https://github.com/XCraftTM/XC-Circuit/issues/new?template=feature_request.yml).
- **Security problems:** please report them privately; see [SECURITY.md](SECURITY.md).

## Pull requests

XC Circuit is **source-available, not open source**: the [licence](LICENSE) lets you fork and change the code **to propose changes back** as pull requests, not to publish your own builds. By opening a pull request you agree that your contribution is licensed to XCraftTM under the licence's contribution terms.

Good first contributions:

- **Translations:** `plugins/xc.core/locales/<code>.toml`. See [Translate the app](https://xcrafttm.github.io/XC-Circuit/tutorials/translate).
- **Blocks:** a `.js` file in a plugin's `blocks/` folder. See [Your first block](https://xcrafttm.github.io/XC-Circuit/tutorials/first-block) and the [Blocks reference](https://xcrafttm.github.io/XC-Circuit/api/blocks).
- **Starter bots:** a project in `examples/templates/<name>/` with a `template.json`.
- **Docs:** the website lives in `docs/` (VitePress).

## Building

You need [Rust](https://rustup.rs) (stable) and [Node.js](https://nodejs.org) 22 or newer.

```bash
cargo run -p circuit-editor        # the editor
cargo test --workspace             # Rust tests
node runtime/test/smoke.js         # one of the runtime tests (see runtime/test/)
cd docs && npm install && npm run dev   # the website, with live reload
```

On Linux, install the GTK, xkbcommon, D-Bus and ALSA development packages first (see `.github/workflows/build.yml`).

### Where things are

| Folder | What's in it |
| --- | --- |
| `crates/circuit-core` | Projects, workspaces, blocks and their shapes, pin types, backups |
| `crates/circuit-plugins` | Loading plugins: plugin.toml, block files (in QuickJS), build modes, pin types, themes, translations |
| `crates/circuit-runner` | Running bots: bundles, npm packages, the bot process, secrets, export |
| `crates/circuit-editor/src/app` | The window: menus, commands, dialogs, projects, the tab layout, crash logs, updates |
| `crates/circuit-editor/src/canvas` | The canvas (its frame is `show` in `mod.rs`), block layout, wires, notes, the block picker |
| `crates/circuit-editor/src/editor` | The editor's state: the open project, block shapes, undo, preferences, shortcuts, values |
| `crates/circuit-editor/src/bot` | Running, exporting and Bot settings |
| `crates/circuit-editor/src/builders` | The Embed, Layout and Slash Command Builders |
| `crates/circuit-editor/src/plugins` | Editor plugins (their scripts), installing them |
| `crates/circuit-editor/src/pages` | Settings, backups, setup, the tour, the start page, the side bar, the inspector |
| `crates/circuit-editor/src/ui` | Shared pieces: dialogs, widgets, inputs, the theme, fonts, the command palette |
| `crates/circuit-editor/src/i18n` | Interface translations |
| `crates/circuit-editor/src/bench` | Benchmarks (see the website's Performance page) |
| `runtime/` | The Node.js runtime that runs bots (also inside exports) and its tests |
| `plugins/` | Circuit Core, Discord Bot and Fluxer Bot: their blocks, build modes, pin types and runtime libraries |

**Changing blocks:** raise the plugin's `version` in its `plugin.toml` (1.0.0 → 1.0.1 for fixes, 1.1.0 for new blocks), so apps notice the update; the Checks fail otherwise (`python tools/plugin_updates.py check`). Once on `main`, the Plugins workflow publishes it as a block update. If the blocks need something only a newer app has, set `[engines] app = "0.2.0"` too.

Before sending a pull request, please run the tests. If you added interface texts, run the tests with `XC_UPDATE_STRINGS=1` once and add German translations to `plugins/xc.core/locales/de.toml` (or leave them empty; they'll show in English).

## Style

- **Plain English in the interface:** short words, no jargon, as if explaining to a friend. Blocks are written in English.
- **One change per commit,** with a message that says what changed and why.
- Follow the code around you; `cargo clippy` should stay quiet.
