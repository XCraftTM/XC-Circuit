# AGENTS.md

A guide for AI agents (Claude, Codex, Cursor …) working in this repository or on bots made with XC™ Circuit. People are welcome to read it too: the human-facing docs are at https://xcrafttm.github.io/XC-Circuit/ and in [CONTRIBUTING.md](CONTRIBUTING.md).

**The complete documentation**, every page of the website in one file, is [`docs/public/llms-full.txt`](docs/public/llms-full.txt) (about 170 KB; also at https://xcrafttm.github.io/XC-Circuit/llms-full.txt). Read the parts you need instead of loading all of it. `python tools/agent_docs.py` makes it from `docs/`.

## What XC Circuit is

A desktop editor (Rust, egui) for Discord and Fluxer bots. A bot is a **project**: workspaces of **blocks** wired together on a canvas. **Events** (a slash command was used, a member joined) start flows; **actions** (send a message, add a role) run one after another through their Action pins; **values** flow from outputs into other blocks' fields. Pressing Run turns the project into a bundle that a **Node.js runtime** runs with discord.js; Export writes a folder that runs with `node index.js`. Blocks, build modes, themes, translations and editor features come from **plugins** written in JavaScript.

There are two ways an agent works with it:

1. **On the code**: this repository. See [Repository map](#repository-map), [How it fits together](#how-it-fits-together) and [Rules for changes](#rules-for-changes).
2. **On a user's bot**, through XC Circuit's **MCP server**, while the app is open. See [The MCP server](#the-mcp-server).

## The MCP server

The bundled **AI agents (MCP)** plugin (`plugins/xc.agents`) lets agents edit the bot that is open in the app. Users add it with **Plugins › AI agents panel** (one click for Claude Desktop, Claude Code, Cursor, VS Code) or by hand:

```json
{ "mcpServers": { "xc-circuit": { "command": "<path to xc-circuit or xc-circuit.exe>", "args": ["--mcp"] } } }
```

`xc-circuit --mcp` speaks MCP over stdio and passes calls to the running app (`127.0.0.1`, with a token in the app's data folder). If the app isn't open, the only tool is `open_xc_circuit`; when the app opens or closes, the server sends `notifications/tools/list_changed`, so clients list the tools again.

Tools, and how to use them well:

| Tool | Use |
| --- | --- |
| `get_project` | Start here: workspaces (key, name, folder, block count), problems, slash commands, whether the bot runs. |
| `get_workspace` | Blocks (id, type, values, pins), wires (`"1.next > 2.in"`), notes. |
| `find_block_types` · `get_block_type` | Find blocks by words; read a type's fields, outputs and flows **before** adding one. Embed, layout and canvas fields carry a `format`. |
| `get_block` | One block as it is now, with every pin. A block's `define` adds pins from its settings (`${name}` in a text becomes an input pin), so read this after setting values. |
| `add_block` · `set_block_values` · `move_block` · `delete_blocks` | Change blocks. Embeds and layouts can be given as Discord's own JSON. |
| `connect` · `disconnect` | Wires, checked like drawn ones. Flow: `<block>.next` (or another flow such as `failed`) to `<block>.in`. Value: `<block>.<output>` to `<block>.<field>`; the field becomes a pin. |
| `add_note` · `set_note` · `delete_note` | Notes: framed areas with a title and text; `around: [block ids]` frames blocks. |
| `create_workspace` · `get_slash_commands` · `set_slash_commands` | Workspaces and the project's slash commands (`set` replaces the whole list). |
| `open_builder` · `show_in_editor` | Show the user: open the Embed/Layout/Canvas Builder or Slash Commands, or centre the canvas on a block (at a `zoom` in percent, if given). |
| `screenshot` | See the canvas as the user does (a workspace fitted to view, or centred on a block), at a `zoom` in percent (25 to 200) when given. Check layouts with it: blocks with previews (embeds, layouts, pictures) are tall. |
| `get_problems` · `get_console` | What's wrong; what the bot logged (with the block that wrote it). |
| `list_block_types` · `get_block_source` · `list_plugins` | Every block type by category; a block type's source code (its JavaScript file), to see exactly what it does; the plugins and their blocks. |
| `get_app_settings` · `get_bot_settings` · `set_bot_settings` | The app's settings (reading only); Bot › Settings, changed with checks like the page's. Secrets only say whether one is saved and can't be set. |
| `get_server` · `server_action` · `set_run_target` | The bot's server (Bot › Remote…: SFTP or Calagopus): its state, CPU and memory, the last upload, its console; upload, start, restart, stop, kill; whether `run_bot` acts here or there. |
| `run_bot` · `stop_bot` · `save_project` | Run and save (on the server when the run target is server). |
| `run_script` | Many steps in one call: JavaScript with `tools` (every tool as a function: `tools.add_block({ … })`), `xc` (the [editor plugin API](docs/api/editor.md)) and `print()`. Sandboxed, 5 seconds. |

Good to know:

- `get_workspace` gives each block's drawn `size`; `add_block` without x and y finds a free spot, and warns when a given position overlaps blocks.
- A typical slash command: `discord.on_slash_command` (field `command`) → actions → `discord.send_message` (mode `reply`), with the event's `interaction` output wired to Send Message's `interaction` input. The command must also exist in `set_slash_commands`.
- Texts take `${…}`: JavaScript, with values from earlier blocks by name.
- Value blocks (Get Property, Math, Compare …) have `in` and `next` too: put them in the flow to make clear when they run (once, there); left out of it, they're worked out when a block reads them.
- Every change shows in the editor at once and can be undone there (Ctrl+Z). The tools never return tokens or other secrets, and the Console hides them (`[hidden secret]`): don't make the bot print or send them.
- A project's server (Bot › Remote…) and its own plugins are the user's to confirm: `xc.data.set("hosting", …)` is refused, `get_server`'s `savedHere` says whether uploads go anywhere, and plugins in a project's `plugins/` folder load only after the user said yes.
- Plugins can add their own tools with `xc.tool({ name, description, input, run })`.

## Repository map

| Path | What's there |
| --- | --- |
| `crates/circuit-core` | The model, independent of Discord: projects and their files (`project.rs`), workspaces, nodes, links, zones, notes (`graph.rs`), block definitions and shapes (`block.rs`), pin types (`types.rs`), `${…}` templates, backups. |
| `crates/circuit-plugins` | Loading plugins: `plugin.toml` (`manifest.rs`), the QuickJS sandbox that reads block files and runs their `define` (`engine.rs`), the registry of blocks, build modes, pin types, themes and translations, TypeScript stripping. |
| `crates/circuit-runner` | Running bots: the bundle (`bundle.rs`), npm packages and Node.js (`node.rs`, `node_setup.rs`), the bot process and its debug protocol (`session.rs`, `protocol.rs`), secrets in the OS credential store (`secrets.rs`), export, database browsing. |
| `crates/circuit-sync` | A project in a Git repository (libgit2, built in, HTTPS): store, send, fetch, and putting two versions together by what the files hold (`json.rs`). `live.rs`: the exchange of a live session, as two state machines (host and guest) without network or window. |
| `crates/circuit-editor/src/app` | The window: menus, commands, dialogs, the dock layout (`dock.rs`), projects, plugins, updates, crash logs. |
| `…/src/canvas` | The canvas: `show` in `mod.rs` draws a workspace; `layout.rs` places a block's rows and pins; wires, notes, the block picker. |
| `…/src/editor` | Editor state: the open project, block shapes, undo history, preferences, shortcuts, values for `${…}` completion. |
| `…/src/builders` | Embed, Layout, Canvas and Slash Command Builders (their tabs, previews and JSON views). |
| `…/src/plugins` | Editor plugins: the script sandbox (`script.rs` + `script_prelude.js`, the `xc` API), what plugins see of the editor (`api.rs`, `host.rs`), installing, updates of bundled blocks. |
| `…/src/mcp` | The MCP server: `stdio.rs` (`--mcp`), `bridge.rs` (the app's local link), `install.rs` (adding it to AI clients). |
| `…/src/pages` | Side bar, inspector, panels (Console, Problems), Settings, backups, setup, tour, start page, Stored Data. |
| `…/src/ui` | Shared pieces: the own dock (`ui/dock`), dialogs, widgets, inputs, theme, fonts, the command palette. |
| `…/src/i18n` | Interface translations; `ui_strings.rs` is generated (see below). |
| `runtime/circuit-runtime.js` | The Node.js runtime that runs a bundle (Run, and inside exports). `runtime/test/*.js`: its tests. |
| `plugins/xc.core` | Core blocks (logic, values, stored data, timers, databases …), the interface's translations (`locales/`), themes, icons, settings pages. |
| `plugins/xc.discord` · `plugins/xc.fluxer` | Their build modes (`modes/`), blocks, pin types (`types/`), runtime libraries (`runtime/`). |
| `plugins/xc.server` · `plugins/xc.agents` | Server tools blocks · the AI agents plugin (MCP tools and its panel). |
| `examples/DemoBot` · `examples/templates/*` | The example bot and the New Project starters (each with a `template.json`). |
| `host/` | The XC Circuit Host: the container people run themselves (`server.js`, no dependencies beyond Node.js 24 and Git). Modes `full` (projects as Git repositories, paired devices, a vault of secrets the devices encrypt) and `relay` (only passes live sessions on). Tests: `node host/test/host.js`. |
| `sdk/` | TypeScript types for block files and editor plugins (`index.d.ts`, `editor.d.ts`). |
| `docs/` | The website (VitePress). `docs/api/` is the plugin reference; `docs/explanations/` how things work. |
| `tools/` | `plugin_updates.py` (bundled plugin versions and update packs), `agent_docs.py` (llms-full.txt), `third_party_notices.py`. |
| `packaging/` · `.github/workflows/` | Installers and icons · CI: `checks.yml` (tests, Clippy, plugin versions, docs file, website), `build.yml` (releases on a `v*` tag), `plugins.yml`, `website.yml`. |

## How it fits together

1. **Loading.** The editor finds plugin folders (bundled `plugins/`, the user's, a project's own). `circuit-plugins` reads each `plugin.toml`, runs block files in QuickJS to read their definitions, and builds a registry of blocks, build modes, pin types, themes and translations.
2. **Editing.** A project is opened from disk (`circuit-core`). The canvas draws each workspace; a block's **shape** (its fields, outputs and flows) comes from its definition plus its `define(values)`, cached in `editor/shapes.rs`. Edits change the project in memory; the undo history records them; saving writes JSON files.
3. **Editor plugins** (`[editor] script` in plugin.toml) run in their own QuickJS context with the `xc` object: commands, panels, checks, overlays, tools for agents. They reach the editor only through `api::Host` (`plugins/host.rs`).
4. **Running.** `circuit-runner` builds a bundle (the project's graph, the blocks' code, settings, secrets from the credential store), installs npm packages, and starts `node runtime/circuit-runtime.js --bundle … --debug`. The runtime loads the build mode's runtime library (discord.js for Discord), starts the events and runs blocks; with `--debug` it reports to the editor over stdout (`@@xc` lines), which lights blocks up and fills the Console.
5. **Exporting** writes the same bundle and runtime as a Node.js project.

## Data and file formats

- **Project folder (layout 2):** `circuit.project.json` (`layout`, `id`, name, mode, plugin versions, shared settings, workspace `folders`), `commands/<name>.json` (one slash command each) and `workspaces/<key>.json`. `.circuit/` is this computer's part and stays out of Git: `local.json` (where the bot is uploaded to, database connections, private settings), the run folder, the bot's stored data. In memory (`ProjectMeta`), for plugins, agents and the bundle, the project is whole: `settings` and `data` (with `slashCommands` and `hosting`) hold both parts; only `Project::load` and `Project::save` in `circuit-core/src/project.rs` know the files. Layout 1 (`circuit.json`) is converted when opened. Full reference: [docs/api/project-format.md](docs/api/project-format.md).
- **Workspace JSON:** `{ name, folder?, nodes: { "<id>": { block: "type@major", at: [x, y], values?, pins?, zone?, width?, heights? } }, links: ["1.next > 2.in", "3.result > 4.text"], zones?, notes?: { "n1": { title, text?, rect: [x, y, w, h], color? } }, disabled? }`. The action input pin is called `in`.
- **Block file** (`plugins/<id>/blocks/**/*.js`): `module.exports = { id, kind: "event" | "action" | "value", info: { name, description, version }, search, look, fields: { key: { type, default, pin, choices, template, … } }, outputs, flows?, define?(…), run({ fields, ctx }) }`. Reference: [docs/api/blocks.md](docs/api/blocks.md), [fields.md](docs/api/fields.md), [ctx.md](docs/api/ctx.md). Example: `plugins/xc.core/blocks/*/log.js`.
- **plugin.toml:** id, name, version, `[engines]` (`app` = the oldest app version it needs), `[dependencies]`, `[contributes]` (blocks, modes, types, themes, locales, botSettings …), `[editor] script`, `optional`. Reference: [docs/api/plugin-toml.md](docs/api/plugin-toml.md).
- **Builders' values** (kept in block fields): embeds `{ color: "#RRGGBB", title, description, fields, author, footer, image, thumbnail, timestamp }`; layouts Discord's Components V2 JSON `{ components: [ … ] }`; canvases `{ width, height, background, layers: [ … ] }` (`builders/canvas.rs`, `Layer`).
- **Secrets** (bot tokens, database passwords) live in the OS credential store, never in project files. **App data** (preferences, the user's plugins, project backups, `mcp.json`): `%APPDATA%\XC Circuit` on Windows, `~/.local/share/xc-circuit` on Linux; `XC_CIRCUIT_DATA` overrides it.

## Build and test

```bash
cargo run -p circuit-editor                        # the app
cargo test --workspace                             # Rust tests
cargo clippy --workspace --all-targets -- -D warnings
node runtime/test/smoke.js                         # runtime tests: each file in runtime/test/ (see checks.yml for the list)
python tools/plugin_updates.py check               # a changed bundled plugin needs a higher version
python tools/agent_docs.py check                   # docs/public/llms-full.txt matches docs/
cd docs && npm install && npm run build            # the website
```

`XC_CIRCUIT_DATA`, `XC_CIRCUIT_PROJECT` and `XC_CIRCUIT_PLUGINS` point the app at another data folder, project or plugin folder (useful for tests). On Windows a running `xc-circuit.exe` (including an AI client's `--mcp` server) locks the file: close the app, or rename the old exe, before rebuilding.

## Rules for changes

- **Plain English** in the interface and docs: short words, no jargon, as if explaining to a friend. **Blocks stay English**; the interface is translated.
- **Interface texts** use `t!("…")` / `tf!("… {name}", name = …)`. After adding or changing any, run `XC_UPDATE_STRINGS=1 cargo test -p circuit-editor the_string_list` and add German translations to `plugins/xc.core/locales/de.toml`; the German tests fail on missing or stale entries. Plugins translate their own texts in their `locales/*.toml` (`xc.t(…)` in editor scripts).
- **Bundled plugins:** raise `version` in `plugin.toml` for every change (translations included), or apps won't get the update. If it needs something only a newer app has, set `[engines] app` to the next app version and raise the workspace version in `Cargo.toml` to match.
- **Keep it working and tested:** `cargo test`, Clippy with `-D warnings` and the runtime tests pass before committing; new behaviour gets a test (`crates/circuit-editor/src/plugins/host.rs` shows how to drive editor plugins and agent tools).
- **CHANGELOG.md:** user-visible changes go under `## Unreleased` (Added, Changed, Fixed), in plain words.
- **Docs:** update `docs/` with the feature, then `python tools/agent_docs.py`.
- **Commits:** one change per commit, saying what changed and why. Don't reformat files you didn't change (no `cargo fmt` over the tree).
- **Never** put tokens or secrets in project files, logs or test fixtures; the runtime and the MCP tools must not expose them.
