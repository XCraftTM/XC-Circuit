# plugin.toml

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
| `overrides` | list of block ids | Blocks of other plugins this plugin may replace with blocks of the same id. Without it, a second block with an id that exists is refused. |

## `[engines]`

| Key | Description |
| --- | --- |
| `circuit` | The plugin API versions the plugin works with: `"^1.0"`, `"1.x"` or `">=1.0"`. Plugins for another major version aren't loaded. Missing means any. |

## `[dependencies]`

Other plugins this one needs, by id, with a version range: `"xc.core" = "^1.0"`. They are loaded first; a plugin whose dependencies are missing isn't loaded, and the Problems panel says why.

These are **plugins**. npm packages for the bot go in a block's [`requires.dependencies`](./blocks#requires) or a build mode's [`[dependencies]`](./build-modes#dependencies).

## `[contributes]`

Each entry is a list of paths or glob patterns relative to the plugin folder. `**` matches any folders, `{a,b}` either word.

| Key | Contributes | Reference |
| --- | --- | --- |
| `blocks` | block files | [Blocks](./blocks) |
| `buildModes` | build modes | [Build modes](./build-modes) |
| `pinTypes` | pin types | [Pin types](./pin-types) |
| `classes` | class catalogs: the members of the values behind pin types, for completion in `${…}` | [Pin types](./pin-types#class-catalogs) |
| `themes` | themes | [Themes](./themes) |
| `locales` | translations | [Translations](./locales) |
| `icons` | icons, by file name without extension (`icons/rocket.svg` is `rocket`) | — |

Runtime libraries are named by the build mode that uses them ([`runtimeLibs`](./build-modes#keys)), and templates are found in the plugin's `templates/` folder without being listed.

## `[editor]`

| Key | Description |
| --- | --- |
| `script` | The editor script, relative to the plugin folder: `editor.js` or `editor.ts`. Default `editor.js`. See [Editor plugins](./editor). |

## `[native]`

| Key | Description |
| --- | --- |
| `library` | The library name without prefix or extension: `my_plugin` means `my_plugin.dll` on Windows, `libmy_plugin.so` on Linux. See [Native plugins](./native). |

A plugin with both `[native]` and `[editor]` runs its native part.

## Where plugins are loaded from

The app's `plugins/` folder, your plugins folder (`%APPDATA%\XC Circuit\plugins` or `~/.local/share/xc-circuit/plugins`), the folders in `XC_CIRCUIT_PLUGINS` (separated like `PATH`), and the open project's `plugins/` folder. Plugin files are watched and reload when saved.
