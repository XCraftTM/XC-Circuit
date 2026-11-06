# API reference

Every key and function plugins can use. New here? Start with the tutorials [Your first block](/tutorials/first-block) and [Your first editor plugin](/tutorials/first-editor-plugin).

## What do you want to do?

| I want to… | Read |
| --- | --- |
| make a **new block** for the picker | [Blocks](./blocks) → [Fields and types](./fields) → [Run context (ctx)](./ctx) |
| add a **panel, command or check** to the editor | [Editor plugins (xc)](./editor) → [Panels (ui)](./panels) |
| make something **float over the editor** (a desk pet) | [Overlays](./editor#xc-overlay) |
| support a **new kind of bot** (another chat app) | [Build modes](./build-modes) → [Runtime libraries](./runtime-libs) → [Pin types](./pin-types) |
| make a **theme** | [Themes](./themes) |
| **translate** the app | [Translations](./locales) |
| offer a **starter bot** in New Project | [Templates](./templates) |
| read or write **project files** with other tools | [Project format](./project-format) |

## Every plugin is a folder

```
my.plugin/
├─ plugin.toml   what it is, what it adds   → plugin.toml
├─ blocks/       blocks (.js, .ts)          → Blocks
├─ editor.js     panels, commands, checks   → Editor plugins
├─ modes/        kinds of bots (.toml)      → Build modes
├─ runtime/      code for a kind of bot     → Runtime libraries
├─ types/        value types (.toml)        → Pin types
├─ themes/       themes (.toml)             → Themes
├─ locales/      translations (.toml)       → Translations
├─ icons/        icons (.svg, .png)
└─ templates/    starter bots               → Templates
```

Only [`plugin.toml`](./plugin-toml) is required. Share a plugin as a `.zip` of its folder: people install it with **Plugins › Install from Zip…**

## Types for your code editor

The repository's [`sdk/`](https://github.com/XCraftTM/XC-Circuit/tree/main/sdk) folder is the `@xc-circuit/sdk` package:

```ts
import { defineBlock } from "@xc-circuit/sdk";        // blocks
/// <reference types="@xc-circuit/sdk/editor" />      // editor plugins (xc, ui)
```

## Versions

The plugin API is version **1.0**. A plugin says which versions it works with in `plugin.toml` (`[engines] circuit = "^1.0"`); plugins for another major version aren't loaded. Additions in 1.x keep old plugins working.
