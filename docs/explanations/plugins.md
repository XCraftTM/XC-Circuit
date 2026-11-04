# Plugins

Almost everything in XC™ Circuit comes from plugins: the blocks, the Discord and Fluxer build modes, their pin types, the themes, the translations, the starters. The editor itself only knows how to load plugins, draw graphs and run bots. Even the bundled blocks are ordinary plugin files you can read in `plugins/`.

## What a plugin is

A folder with a [`plugin.toml`](/api/plugin-toml) that says what it contributes:

| Part | What it adds | Runs in |
| --- | --- | --- |
| [Blocks](/api/blocks) | blocks for the picker | the bot (and `define()` in the editor) |
| [Build modes](/api/build-modes) | a kind of bot: blocks, categories, settings, npm packages | — |
| [Runtime libraries](/api/runtime-libs) | shared code for a mode's blocks, such as the Discord client | the bot |
| [Pin types](/api/pin-types) | value types with colours and conversions | the bot (checks) and the editor |
| [Themes](/api/themes), [translations](/api/locales), [templates](/api/templates) | looks, languages, starters | the editor |
| [Editor script](/api/editor) | panels, commands, checks | the editor, sandboxed |
| [Native library](/api/native) | panels, commands, checks in Rust | the editor, with full access |

## Where plugins come from

XC Circuit loads plugins from:

- Folders listed in the `XC_CIRCUIT_PLUGINS` environment variable (handy while developing plugins).
- The **app's own** `plugins/` folder (Circuit Core, Discord Bot, Fluxer Bot).
- **Your plugins folder**: `%APPDATA%\XC Circuit\plugins` on Windows, `~/.local/share/xc-circuit/plugins` on Linux. Plugins › Open folder opens it.
- The open **project's** `plugins/` folder: plugins that belong to one bot and travel with it.

Plugin files are watched: saving a block, a theme or an editor script reloads it right away.

## Two kinds of editor code

Plugins can change the editor in two ways, and they're trusted differently.

**Editor scripts (JavaScript)** are the normal way. Each runs in its own sandbox with a time and memory limit, and can only reach the app through the [`xc` object](/api/editor): read and change the project, add panels and commands, log. No files, no network, no programs. Because of that, scripts start without asking, also in a project you downloaded.

**Native plugins (Rust)** are compiled libraries loaded into the editor. They draw with egui directly and can do anything the app can, including reading your files. That's why they're **off** until you allow native plugins (Plugins › Manage Plugins), and plugins outside the app's own folder must also be allowed one by one. A native plugin must also be built with exactly the same compiler and libraries as the app. Use them only when a script can't do what you need.

## Bot code is different

Blocks and runtime libraries don't run in the editor; they run in your **bot**, a Node.js program with the same access as any other program. That's the point: a bot needs the network, files and npm packages. Only use block plugins from people you trust, as you would with any npm package.
