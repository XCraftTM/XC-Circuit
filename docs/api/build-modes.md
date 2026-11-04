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

[settings]
token  = { type = "secret", group = "Connection", label = "Bot token", required = true }
prefix = { type = "text", group = "Messages", label = "Text command prefix", default = "!", pattern = '\S{1,5}', patternHint = "1 to 5 characters, without spaces." }

[dependencies]
"@fluxerjs/core" = "^3.3.0"
```

## Keys

| Key | Description |
| --- | --- |
| `id` | **Required.** Unique id, saved in projects' `circuit.json` as `mode`. |
| `name` | Shown in File › New Project. |
| `icon` | An icon name. |
| `description` | One sentence, shown in File › New Project. |
| `blocks` | Which blocks appear: `"plugin.id/*"` for all blocks of a plugin. |
| `runtime` | `"node"`. |
| `runtimeLibs` | [Runtime libraries](./runtime-libs), as `"plugin.id/path/in/plugin.js"`. |
| `exporter` | How Bot › Export packages the bot (`"<plugin>/node-bundle"`: a Node.js folder). |
| `tokenEnv` | The environment variable an exported bot reads its token from. Default `DISCORD_TOKEN`. |
| `features` | What the mode's projects have. `"slashCommands"` shows the Slash Command Builder. |

## categories

The block picker's categories, in order. Blocks name one in [`search.category`](./blocks#search).

| Key | Description |
| --- | --- |
| `id` | What blocks refer to. |
| `name` | Shown in the picker (translatable). |
| `icon` | An icon name. |

The Discord mode's categories: `bot`, `commands`, `components`, `send`, `messages`, `design`, `reactions`, `members`, `moderation`, `roles`, `channels`, `server`, `voice`, `premium`, `format`, `flow`, `calls`, `time`, `variables`, `data`, `text`, `numbers`, `lists`, `web`, `code`. Fluxer's are similar, without those Fluxer doesn't have yet (slash commands, components).

## settings

The fields of Bot › Settings, by key. Runtime libraries and blocks read them with `rt.setting(key)` / [`ctx.runtime.setting(key)`](./ctx#runtime).

| Key | Description |
| --- | --- |
| `type` | `"text"`, or `"secret"`: kept in the credential store, never in the project, read with `rt.secret(key)`. The key `token` is the bot token. |
| `group` | A heading the setting is listed under. |
| `label` | Its name. |
| `description` | Help under it. |
| `placeholder` | Grey hint in the empty box. |
| `default` | The value until one is set. |
| `required` | Marked with * in Bot › Settings. |
| `pattern` | A regular expression the value must match; checked as you type. |
| `patternHint` | What to say when it doesn't. |
| `sensitive` | `true` hides a text setting behind the eye button, like a secret. |

Secrets other than the token reach an exported bot through `.env` as `XC_SECRET_<KEY>` (`XC_SECRET_SPOTIFYCLIENTSECRET`).

## dependencies

npm packages the runtime libraries need, installed before Run and listed in an export's `package.json`. Blocks add their own with [`requires.dependencies`](./blocks#requires).
