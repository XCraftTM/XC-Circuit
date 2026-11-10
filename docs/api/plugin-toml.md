# plugin.toml

::: info In short
The file that makes a folder a plugin: its `id`, `name` and `version`, and what it adds (`[contributes]`, `[editor]`).
:::

Every plugin folder has a `plugin.toml` at its top. It says what the plugin is and what it contributes.

```toml
id          = "acme.tools"
name        = "Acme Tools"
version     = "1.2.0"
author      = "Acme"
description = "Blocks and a panel for Acme bots."
icon        = "icons/acme.svg"

[engines]
circuit = "^1.0"

[dependencies]
"xc.core" = "^1.0"

[contributes]
blocks  = ["blocks/**/*.{js,ts}"]
themes  = ["themes/*.toml"]
locales = ["locales/*.toml"]

[editor]
script = "editor.js"
```

## Top level

| Key | Type | Description |
| --- | --- | --- |
| `id` | text | **Required.** Unique, lowercase, dotted: `you.plugin`. Block ids usually start with it. |
| `name` | text | **Required.** Shown in the Plugins view and the manager. |
| `version` | text | The plugin's version, `major.minor.patch`. |
| `author` | text | Who made it. |
| `description` | text | One or two sentences, shown under the name. |
| `icon` | path | An SVG or PNG in the plugin folder. |
| `optional` | true / false | For plugins that come with the app: it can be turned off and removed (and added back), unlike the others. |
| `overrides` | list of block ids | Blocks of other plugins this plugin may replace with blocks of the same id. Without it, a second block with an id that exists is refused. |

## `[engines]`

| Key | Description |
| --- | --- |
| `circuit` | The plugin API versions the plugin works with: `"^1.0"`, `"1.x"` or `">=1.0"`. Plugins for another major version aren't loaded. Missing means any. |
| `app` | The oldest XC Circuit version the plugin works with: `"0.2.0"`. Older apps don't load it (and don't download it as a block update). Set it when the plugin's blocks need something only newer apps have. Missing means any. |

When the same plugin (by `id`) is in two folders, the one with the higher `version` loads.

## `[dependencies]`

Other plugins this one needs, by id, with a version range: `"xc.core" = "^1.0"`. They are loaded first; a plugin whose dependencies are missing isn't loaded, and the Problems panel says why.

These are **plugins**. npm packages for the bot go in a block's [`requires.dependencies`](./blocks#requires) or a build mode's [`[dependencies]`](./build-modes#dependencies).

## `[contributes]`

Each entry is a list of paths or glob patterns relative to the plugin folder. `**` matches any folders, `{a,b}` either word. They stay inside the plugin's folder: a pattern that starts at the top of the disk or goes up with `..` is left out, with a line in Problems.

| Key | Contributes | Reference |
| --- | --- | --- |
| `blocks` | block files | [Blocks](./blocks) |
| `blockModes` | the kinds of bot your blocks are offered in, by build mode id: `["discord-bot"]`. Leave it out and they are offered in every kind | [below](#where-your-blocks-show) |
| `buildModes` | build modes | [Build modes](./build-modes) |
| `pinTypes` | pin types | [Pin types](./pin-types) |
| `classes` | class catalogs: the members of the values behind pin types, for completion in `${…}` | [Pin types](./pin-types#class-catalogs) |
| `themes` | themes | [Themes](./themes) |
| `locales` | translations | [Translations](./locales) |
| `botSettings` | pages and settings for Bot › Settings | [Plugin settings](./build-modes#plugin-settings) |
| `icons` | icons, by file name without extension (`icons/rocket.svg` is `rocket`) | — |

Runtime libraries are named by the build mode that uses them ([`runtimeLibs`](./build-modes#keys)), and templates are found in the plugin's `templates/` folder without being listed.

### Where your blocks show

A plugin that adds blocks needs no build mode of its own, and no change to anyone else's: its blocks are in the block picker of **every kind of bot** (Discord Bot, Fluxer Bot …), in the category each names in [`search.category`](./blocks#search).

Blocks that only make sense for one kind (they use discord.js, say) name it:

```toml
[contributes]
blocks     = ["blocks/**/*.js"]
blockModes = ["discord-bot"]
```

The ids are the build modes' (`discord-bot`, `fluxer-bot`, or another plugin's). Only a plugin that a [build mode](./build-modes) lists in its `blocks` belongs to that kind of bot alone: that is how Discord's own blocks stay out of Fluxer bots.

## `[editor]`

| Key | Description |
| --- | --- |
| `script` | The editor script, relative to the plugin folder: `editor.js` or `editor.ts`. Default `editor.js`. See [Editor plugins](./editor). |

A `[native]` section (a compiled Rust library) is no longer loaded: XC Circuit runs editor code written in JavaScript only. Such a plugin's blocks and settings still load, and the Plugins view says its native part isn't supported.

## Where plugins are loaded from

The app's `plugins/` folder, your plugins folder (`%APPDATA%\XC Circuit\plugins` or `~/.local/share/xc-circuit/plugins`), the folders in `XC_CIRCUIT_PLUGINS` (separated like `PATH`), and the open project's `plugins/` folder. Plugin files are watched and reload when saved.
