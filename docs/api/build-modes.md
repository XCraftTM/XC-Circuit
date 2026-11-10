# Build modes

::: info In short
A build mode is a kind of bot (Discord Bot, Fluxer Bot): which blocks it offers, the picker's categories, the settings in Bot › Settings and the npm packages it needs.
:::

A build mode is a kind of bot: *Discord Bot*, *Fluxer Bot*. It decides which blocks the picker offers and how they're sorted, which settings Bot › Settings asks for, which npm packages the bot needs, and which [runtime libraries](./runtime-libs) set the bot up. Modes are TOML files listed in a plugin's `contributes.buildModes`.

```toml
id          = "fluxer-bot"
name        = "Fluxer Bot"
icon        = "message"
description = "A bot for Fluxer, built with fluxer.js."

blocks      = ["xc.core/*", "xc.fluxer/*"]
runtime     = "node"
runtimeLibs = ["xc.fluxer/runtime/fluxer.js"]
exporter    = "xc.fluxer/node-bundle"
tokenEnv    = "FLUXER_BOT_TOKEN"

[[categories]]
id = "send"
name = "Send Messages"
icon = "send"

[[pages]]
id = "connection"
name = "Connection"
icon = "plug"
description = "How your bot logs in to Fluxer."
order = 10

[settings]
token  = { type = "secret", group = "Connection", label = "Bot token", required = true }
prefix = { type = "text", group = "Messages", label = "Text command prefix", default = "!", pattern = '\S{1,5}', patternHint = "1 to 5 characters, without spaces." }

[dependencies]
"@fluxerjs/core" = "^3.3.0"
```

## Keys

| Key | Description |
| --- | --- |
| `id` | **Required.** Unique id, saved in projects' `circuit.project.json` as `mode`. |
| `name` | Shown in File › New Project. |
| `icon` | An icon name. |
| `description` | One sentence, shown in File › New Project. |
| `blocks` | Which blocks appear: `"plugin.id/*"` for all blocks of a plugin. |
| `runtime` | `"node"`. |
| `runtimeLibs` | [Runtime libraries](./runtime-libs), as `"plugin.id/path/in/plugin.js"`: a file inside that plugin's folder. |
| `exporter` | How Bot › Export packages the bot (`"<plugin>/node-bundle"`: a Node.js folder). |
| `tokenEnv` | The environment variable an exported bot reads its token from. Default `DISCORD_TOKEN`. |
| `features` | What the mode's projects have. `"slashCommands"` shows the Slash Command Builder. |
| `privilegedIntents` | A table of intents (as blocks name them) that must be turned on by hand, with what the platform calls each: Bot settings › Privileged intents lists the ones the project's blocks need. |
| `intentsPage` | Where they are turned on; `{appId}` is the bot's application id (from its token). A web address (`https://…`), as `inviteUrl` is: nothing else is opened. |
| `inviteUrl` | A link that adds the bot to a server, offered under the bot token once one is set: `{appId}` is the bot's application id, `{permissions}` the number picked from `invitePermissions`. |
| `invitePermissions` | A table of the permissions to choose from for `inviteUrl`: name (translatable) → the permissions number. The first is picked at first. |

## categories

The block picker's categories, in order. Blocks name one in [`search.category`](./blocks#search).

| Key | Description |
| --- | --- |
| `id` | What blocks refer to. |
| `name` | Shown in the picker (translatable). |
| `icon` | An icon name. |

The Discord mode's categories: `bot`, `commands`, `components`, `send`, `messages`, `design`, `reactions`, `members`, `moderation`, `roles`, `channels`, `server`, `voice`, `premium`, `format`, `flow`, `calls`, `time`, `variables`, `data`, `text`, `numbers`, `lists`, `web`, `code`. Fluxer's are similar, without those Fluxer doesn't have yet (slash commands, components).

## pages

Bot › Settings is a list of pages on the left, like the app's Settings, with the page shown on the right. A mode lists its pages here; [plugins add their own](#plugin-settings).

| Key | Description |
| --- | --- |
| `id` | **Required.** What settings name in `page` (or match with `group`, see below). |
| `name` | Shown in the list and as the page's title (translatable). |
| `icon` | An icon name. |
| `description` | One sentence under the title (translatable). |
| `order` | Where it goes in the list: lower first. Without one it counts as 50; pages with the same order keep the order they're listed in, the mode's first. |

Pages without settings aren't shown. The Discord mode's pages are `connection` (10), `slash-commands` (20), `messages` (30) and `music` (40); Bot › Settings adds **Privileged intents** last for modes that have them.

## settings

The fields of Bot › Settings, by key. Runtime libraries and blocks read them with `rt.setting(key)` / [`ctx.runtime.setting(key)`](./ctx#runtime), and get the `default` until another value is saved.

| Key | Description |
| --- | --- |
| `type` | `"text"` (the default), `"secret"`, `"select"`, `"boolean"`, `"number"`, or one of the [elements](#elements) below. A secret is kept in the credential store, never in the project, and read with `rt.secret(key)`; the key `token` is the bot token. |
| `page` | The `id` of the page it's on. |
| `group` | With `page`: a heading on the page. Without `page`: the page itself, the one whose `id` is the group's name in lower case with dashes (`Slash commands` is `slash-commands`); a page is made for it when there's none. Without both, the setting is on *General*. |
| `label` | Its name. |
| `description` | Help under it. |
| `placeholder` | Grey hint in the empty box. |
| `default` | The value until one is set. |
| `required` | Marked with * in Bot › Settings. |
| `pattern` | A regular expression the value must match; checked as you type. |
| `patternHint` | What to say when it doesn't. |
| `sensitive` | `true` hides a text setting behind the eye button, like a secret. |
| `private` | `true` for a value that belongs to whoever set it, not to the bot: the address of a server of theirs, an id of an account of theirs, a safety switch. It is saved on their computer (`.circuit/local.json`), not in the shared project, so sharing or syncing a project neither gives it away nor turns a switch on for someone else. The bot gets it like any other setting. Secrets are private anyway. |
| `choices` | For `select`: the values and what the list shows, `{ json = "JSON file", sqlite = "SQLite" }`, in that order. |
| `min`, `max` | For `number` and `slider`: the smallest and largest value. |

Texts, secrets and numbers are boxes with the name above them; switches (`boolean`) and lists (`select`) sit on the right of their name. A page whose required setting has no value shows how many in the list; one with a value that doesn't fit shows that in red, and Save waits until it's fixed. An unchanged default isn't written into the project.

Secrets other than the token reach an exported bot through `.env` as `XC_SECRET_<KEY>` (`XC_SECRET_SPOTIFYCLIENTSECRET`).

### Elements

Besides texts, secrets, choices, switches and numbers, a page can hold these. XC Circuit draws them with its UI kit, the same buttons, fields, sliders and colours its own pages use, so a plugin's page looks and behaves like the rest of the app, in every theme.

| `type` | What it is | Its own keys | The value a bot gets |
| --- | --- | --- | --- |
| `textarea` | Several lines of text | `rows` (2 to 20, default 4) | Text |
| `color` | A colour picker | — | `"#RRGGBB"` |
| `slider` | A slider with the number beside it | `min` (0), `max` (100), `step`, `suffix` (`" XP"`) | A number |
| `list` | A list of texts, with + Add and a remove button on each | `placeholder` for each | A list of texts, `[]` when empty |
| `path` | A file (or, with `pick = "folder"`, a folder) on this computer, with Browse… | `pick` | Text |
| `note` | A box of text, no value | `tone`: `"info"` (default), `"warning"` or `"success"`; `label` is its title (optional) | — |
| `link` | A button that opens a web page, no value | `url` (`https://` only), `button` (default *Open*) | — |

```toml
[settings]
lvlAbout  = { type = "note", page = "levels", description = "Members earn XP for each message." }
lvlXp     = { type = "slider", page = "levels", label = "XP for a level", min = 50, max = 1000, step = 50, default = 100, suffix = " XP" }
lvlColor  = { type = "color", page = "levels", label = "Rank card colour", default = "#00D8F0" }
lvlText   = { type = "textarea", page = "levels", label = "Level-up message", rows = 3, default = "GG ${user}!" }
lvlNoXp   = { type = "list", page = "levels", label = "Channels without XP", placeholder = "Channel id", default = [] }
lvlImage  = { type = "path", page = "levels", label = "Card background" }
lvlDocs   = { type = "link", page = "levels", label = "How levels work", url = "https://example.com/levels", button = "Read" }
```

The DemoBot example has a page like this: `plugins/demo.custom/settings/levels.toml`.

## Plugin settings

Any plugin can add pages and settings to Bot › Settings, without a build mode of its own: list TOML files in [`contributes.botSettings`](./plugin-toml#contributes). A file has the same `[[pages]]` and `[settings]` as a mode, and `modes` to show them only for some build modes:

```toml
# plugins/my.database/settings.toml
modes = ["discord-bot", "fluxer-bot"]   # leave out for every mode

[[pages]]
id = "databases"
name = "Databases"
icon = "database"
description = "Where your bot keeps its data."
order = 25

[settings]
dbKind     = { type = "select", page = "databases", group = "Connection", label = "Kind", choices = { mariadb = "MariaDB / MySQL", postgres = "PostgreSQL" }, default = "mariadb" }
dbHost     = { page = "databases", group = "Connection", label = "Host", placeholder = "localhost" }
dbPort     = { type = "number", page = "databases", group = "Connection", label = "Port", default = 3306, min = 1, max = 65535 }
dbPassword = { type = "secret", page = "databases", group = "Connection", label = "Password" }
dbLog      = { type = "boolean", page = "databases", group = "Debugging", label = "Log every query", default = false }
```

Setting keys are shared by the mode and every plugin: one that's taken already isn't shown, and the Problems panel says so. Start yours with something of your own (`dbHost`, `myPluginKey`). Secrets reach a running bot and an exported one's `.env` like the mode's.

## dependencies

npm packages the runtime libraries need, installed before Run and listed in an export's `package.json`. Blocks add their own with [`requires.dependencies`](./blocks#requires).
