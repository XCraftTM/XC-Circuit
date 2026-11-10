# Project format

::: info In short
A project is a folder: `circuit.project.json` (name, mode, shared settings), `commands/*.json` (one slash command each), `workspaces/*.json` (one per canvas: blocks, wires, Try Zones, notes) and `.circuit/local.json` (what stays on your computer).
:::

A project is a folder. All files are JSON, written with stable key order and indentation so they diff well in Git.

```
My Bot/
├─ circuit.project.json
├─ commands/
│  └─ <name>.json
├─ workspaces/
│  └─ <key>.json
├─ plugins/          optional: project plugins
└─ .circuit/         this computer's part (leave out of Git)
   ├─ local.json     where the bot is uploaded to, database connections, private settings
   ├─ data/          the bot's stored data
   └─ run/           the folder the bot runs from
```

This is **layout 2**, written since XC Circuit 0.3.0. What is shared (what the bot is) and what is yours alone (where it runs) are kept apart, so a project can be put into Git or synced without your server's address in it.

A project of layout 1 (one `circuit.json` holding everything) is brought to layout 2 when it is opened; its `circuit.json` is kept in `.circuit/layout-1/`. Older versions of XC Circuit can't open a layout 2 project: they look for `circuit.json` and find none.

## circuit.project.json

```json
{
  "layout": 2,
  "id": "8cd7c5b5ae95edcb6b839ba00c48b6ae",
  "name": "DemoBot",
  "mode": "discord-bot",
  "plugins": { "xc.core": "1.0.0", "xc.discord": "1.0.0" },
  "settings": { "prefix": "!" },
  "folders": ["Commands", "Events"]
}
```

| Key | Description |
| --- | --- |
| `layout` | The version of the project folder's layout, `2`. A version of XC Circuit that only knows older layouts doesn't open the project and says which update it needs; a project of an older layout is brought up to date step by step when it opens. |
| `id` | What tells this project from every other, wherever its folder is. Saved secrets and backups go by it. |
| `name` | The bot's name. |
| `mode` | The [build mode](./build-modes) id. |
| `plugins` | Plugins the project uses, with the versions it was saved with. |
| `settings` | Bot › Settings values that are shared: no secrets, and nothing that says where your databases or servers are. Of `databases` only each one's `id`, `name` and `kind`. |
| `data` | Project-wide data: `slashFolders` (the side bar's folders for slash commands, in order) and anything [editor plugins](./editor#changing-the-project) keep. |
| `folders` | Workspace folders in the order of the Workspaces panel. A folder in a folder is its path with `/`, such as `Tickets/Archive`. |
| `order` | Workspace keys in the side bar's order, when it isn't their files' order (by name). |
| `commandOrder` | Slash commands in the side bar's order, when it isn't their files' order. |

## Commands

Each file in `commands/` is one slash command: a list of its entries. A plain command has one; a command with subcommands has one per subcommand (and one per group).

```json
[
  { "name": "ticket open", "description": "Opens a ticket", "options": [] },
  { "name": "ticket close", "description": "Closes it", "options": [] }
]
```

An entry's `folder` and `disabled` are its side bar folder and whether it's turned off. The file is named after the command (`ticket.json`).

## .circuit/local.json

What belongs to this computer, not to the bot:

```json
{
  "hosting": { "kind": "sftp", "host": "203.0.113.7", "user": "bot", "folder": "/home/bot" },
  "settings": { "devServer": "123456789012345678" },
  "databases": { "main": { "host": "db.example", "port": 5432, "user": "bot" } }
}
```

| Key | Description |
| --- | --- |
| `hosting` | Where Bot › Server… uploads the bot (`kind`, `host`, `port`, `user`, `folder`, `panel`, `server` …; no passwords or keys). |
| `settings` | Private settings, by key. |
| `databases` | Where each database is, by its id: everything of a connection but its `id`, `name` and `kind`. |

In the editor, to plugins and in the running bot the project is whole: `settings` and `data` hold the shared and the local parts together, as before.

## Workspaces

Each file in `workspaces/` is one workspace; its file name without `.json` is its **key**.

```json
{
  "name": "Startup",
  "folder": "Events",
  "nodes": {
    "1": { "block": "core.on_start@1", "at": [40, 60] },
    "2": {
      "block": "core.increase_stored@1",
      "at": [340, 60],
      "values": { "scope": "global", "key": "starts", "by": 1 }
    },
    "3": {
      "block": "core.log@1",
      "at": [660, 60],
      "values": { "message": "Started ${value} times", "level": "success" }
    }
  },
  "links": ["1.next > 2.in", "2.next > 3.in", "2.value > 3.value"]
}
```

| Key | Description |
| --- | --- |
| `name` | Shown in the Workspaces panel. |
| `folder` | The folder it's listed in (a path such as `Tickets/Archive` for a folder in a folder). |
| `nodes` | Blocks by id (the number shown in their title bar). |
| `links` | Wires. |
| `zones` | Try Zones by id. |
| `notes` | Notes on the canvas, by id. [Below](#notes). |
| `disabled` | `true`: the workspace is turned off and left out when the bot runs or is exported. Missing: it is on. |

### Nodes

| Key | Description |
| --- | --- |
| `block` | The block id and its major version: `discord.send_message@3`. |
| `at` | Position on the canvas `[x, y]`. |
| `values` | Field values by key. Missing fields have their default. |
| `pins` | Fields switched to pins. |
| `zone` | The Try Zone the block is in. |
| `heights` | Heights of resized text boxes, by field. |
| `width` | The block's width, when resized. |

### Links

A link is `"from > to"`, each side `node.pin`:

- `"1.next > 2.in"`: an action wire from block 1's `next` output (or one of its `flows`) to block 2's action input `in`.
- `"2.value > 3.value"`: a value wire from block 2's output `value` to block 3's input `value` (a field or a `${…}` name).
- A Try Zone's outputs are wired from the zone id: `"z1.done > 5.in"`, `"z1.error > 6.in"`, `"z1.message > 6.message"`.

### Zones

```json
"zones": { "z1": { "title": "Reply", "rect": [740, 40, 380, 260], "retry": 2, "timeoutMs": 5000 } }
```

| Key | Description |
| --- | --- |
| `title` | Shown on the zone. |
| `rect` | `[x, y, width, height]`, worked out from its blocks. |
| `retry` | How many times to try again. |
| `timeoutMs` | Give up after this many milliseconds. |

### Notes

```json
"notes": { "n1": { "title": "Rank reply", "text": "Builds the embed and replies.", "rect": [0, -80, 1100, 620], "color": "#F5C451" } }
```

| Key | Description |
| --- | --- |
| `title` | Shown in the note's title band. |
| `text` | The description, under the title. |
| `rect` | `[x, y, width, height]` on the canvas. |
| `color` | `#RRGGBB`; missing: the theme's accent. |
| `loose` | `true`: dragging the note leaves the blocks inside where they are (new notes have this). Missing: they move with it. |

Notes don't run: they only explain and group blocks in the editor.
