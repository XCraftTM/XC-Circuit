# API reference

Everything a plugin can contain, key by key. For a guided start, see the tutorials [Your first block](/tutorials/first-block) and [Your first editor plugin](/tutorials/first-editor-plugin); for the ideas behind it, [Plugins](/explanations/plugins).

## A plugin at a glance

```
my.plugin/
├─ plugin.toml          the manifest: id, name, what it contributes
├─ blocks/              block files (.js, .cjs, .mjs, .ts)
├─ editor.js            editor script: panels, commands, checks
├─ modes/               build modes (.toml)
├─ runtime/             runtime libraries for a build mode (.js)
├─ types/               pin types (.toml) and class catalogs (.classes.json)
├─ themes/              themes (.toml)
├─ locales/             translations (.toml)
├─ icons/               icons (.svg, .png)
└─ templates/           starters for File › New Project
```

Only `plugin.toml` is required; everything else is there when the plugin needs it.

## Pages

| Page | What it covers |
| --- | --- |
| [plugin.toml](./plugin-toml) | The manifest |
| [Blocks](./blocks) | Block files: kinds, info, fields, outputs, flows, `run`, `listen`, `define`, `migrate` |
| [Fields and types](./fields) | Field options, type strings, `define()` results |
| [Run context (ctx)](./ctx) | What a block's code can use while the bot runs |
| [Editor plugins (xc)](./editor) | The `xc` object of editor scripts |
| [Panels (ui)](./panels) | Building panel contents |
| [Native plugins (Rust)](./native) | Compiled plugins with egui |
| [Build modes](./build-modes) | Kinds of bots: blocks, categories, settings, packages |
| [Runtime libraries](./runtime-libs) | Shared code for a mode's blocks |
| [Pin types](./pin-types) | Value types, their colours and conversions |
| [Themes](./themes) | Colours and shapes of the editor |
| [Translations](./locales) | Languages for the interface |
| [Templates](./templates) | Starters for new projects |
| [Project format](./project-format) | `circuit.json` and workspace files |

## Versions

The plugin API has a version, currently **1.0**. A plugin says which versions it works with in `engines.circuit` (`"^1.0"`); XC Circuit skips plugins written for another major version. Additions within 1.x keep old plugins working.
