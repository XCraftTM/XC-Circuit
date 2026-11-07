# Project format

::: info In short
A project is a folder: `circuit.json` (name, mode, settings, slash commands) and `workspaces/*.json` (one per canvas: blocks, wires, Try Zones, notes).
:::

A project is a folder. All files are JSON, written with stable key order and indentation so they diff well in Git.

```
My Bot/
├─ circuit.json
├─ workspaces/
│  └─ <key>.json
├─ plugins/          optional: project plugins
└─ .circuit/         the editor's local state, run folder and bot data (leave out of Git)
```

## circuit.json

```json
{
  "format": 1,
  "name": "DemoBot",
  "mode": "discord-bot",
  "plugins": { "xc.core": "1.0.0", "xc.discord": "1.0.0" },
  "settings": { "prefix": "!" },
  "data": { "slashCommands": [] },
  "folders": ["Commands", "Events"]
}
```

| Key | Description |
| --- | --- |
| `format` | The file format version, `1`. |
| `name` | The bot's name. |
| `mode` | The [build mode](./build-modes) id. |
| `plugins` | Plugins the project uses, with the versions it was saved with. |
| `settings` | Bot › Settings values, without secrets. |
| `data` | Project-wide data: `slashCommands` (the Slash Command Builder's, in the side bar's order; an entry's `folder` and `disabled` are its side bar folder and whether it's turned off), `slashFolders` (those folders in order), and anything [editor plugins](./editor#changing-the-project) keep. |
| `folders` | Workspace folders in the order of the Workspaces panel. |

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
| `folder` | The folder it's listed in. |
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
