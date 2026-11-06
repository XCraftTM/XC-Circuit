# Share and update

::: info In short
Raise `version` in `plugin.toml` for every change, zip the plugin folder, and share the zip: people install it with Plugins › Install from Zip… (or by dropping it on the window). Of two copies of a plugin, the higher version loads. Never change a block's `id`; raise its own major version and add `migrate` when its saved values change.
:::

## Before you share

- [ ] `id` is yours and every block and command id starts with it.
- [ ] `name`, `author`, `description` and `version` are filled in: the Plugins view shows them.
- [ ] `[engines] circuit = "^1.0"`, and `app = "…"` if your blocks need something only newer XC Circuit versions have.
- [ ] `[dependencies]` names the plugins yours needs (`"xc.discord" = "^1.0"` for Discord blocks).
- [ ] Every block has `info.name`, `info.description`, `info.author` and `info.version`, and a `search.category`.
- [ ] Nothing in the folder you don't want to give away: no `.env`, tokens, test data or `node_modules`.
- [ ] It loads without problems on a fresh project: the Problems panel is empty.

## Make the zip

Zip the plugin **folder** (so `plugin.toml` is in one folder at the top of the zip) or its **contents** (`plugin.toml` at the top). Both install. GitHub's *Code › Download ZIP* of a repository that is one plugin works too.

```
acme.tools-1.2.0.zip
└─ acme.tools/
   ├─ plugin.toml
   ├─ blocks/…
   └─ editor.js
```

## Installing

**Plugins › Install from Zip…**, the **Install…** button in the Plugins view, or dropping the `.zip` on the window. It goes into your plugins folder and loads right away; the Console says what was installed.

A plugin can also come with a project: one in the project's `plugins/` folder loads whenever the project is open, and goes along when the project is copied or shared. That's the way for a plugin made for one bot.

## Turning off and removing

In the **Plugins view** of the side bar (the plug icon), each plugin you installed has a switch and a trash can:

- **The switch** turns the plugin off: its blocks, panels, commands and themes are gone until you turn it on again. It stays listed. A project that uses its blocks needs it turned on again to work as before.
- **The trash can** removes it: after you confirm, its folder is deleted.

The plugins that come with XC Circuit (Circuit Core, Discord Bot, Fluxer Bot, Server Tools) say *Built in* and can't be turned off or removed.

## Versions

`version` is `major.minor.patch`. Raise it **for every change you share**:

| Change | Raise | Example |
| --- | --- | --- |
| a fix | patch | 1.2.0 → 1.2.1 |
| something new (a block, a panel, a field with a default) | minor | 1.2.1 → 1.3.0 |
| something that breaks what people built | major | 1.3.0 → 2.0.0 |

**Updating is installing the newer zip.** When the same plugin (`id`) is in two places, the higher `version` loads, so a newer copy in the plugins folder wins over an older one in a project, and the other way round.

### Changing a block that's in use

Projects save each block by its `id` and its own **major** version (`acme.count@1`), with the values filled in:

- **Never change a block's `id`**: projects that use it would show it as unknown.
- Adding a field with a `default`, a new output or a better description: no new major version needed.
- Renaming or removing a field, or changing what a value means: raise the block's `info.version` major (`1.4.0` → `2.0.0`) and add [`migrate`](/api/blocks#migrate), which turns old saved values into new ones when a project opens.
- A block you want to retire: `info.deprecated: "Use Count instead"`. It keeps working, but the picker hides it.

### Needing a newer XC Circuit

If your blocks use something new in XC Circuit (a `ctx` member, a field type), set `[engines] app` to the version that has it. Older versions then don't load your plugin, instead of failing while the bot runs.

## Publishing a plugin's code

A Git repository whose top folder is the plugin is the easiest to share: people download the zip from GitHub, and you develop it with [`XC_CIRCUIT_PLUGINS`](./#where-to-keep-it-while-you-work) pointing at the folder above it. Put a `README.md` in the folder; XC Circuit ignores files it doesn't know.
