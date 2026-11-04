# Themes

::: info In short
A TOML file with colours (and a few shapes) for the editor. Picked in Settings › Appearance.
:::

A theme sets the editor's colours and a few shapes. Themes are TOML files listed in a plugin's `contributes.themes`, and are picked in Settings › Appearance. The accent colour can also be changed there, on top of any theme.

```toml
id   = "midnight"
name = "Midnight"
base = "dark"

[colors]
frame       = "#0A0D10"
panel       = "#11161B"
canvas      = "#0D1115"
canvasDots  = "#00D8F014"
node        = "#18212A"
text        = "#E4F1F4"
accent      = "#00E5FF"
action      = "#3DFF6E"

[pinColors]
text = "#F5C451"
"discord.member" = "#8A7BFF"

[shape]
nodeRadius = 9
wireWidth  = 2.6

[motion]
speed = 1.0
```

## Keys

| Key | Description |
| --- | --- |
| `id` | **Required.** Unique id. |
| `name` | Shown in Settings. |
| `base` | `"dark"` or `"light"`: the defaults for colours the theme doesn't set. |

## colors

Colours are `#RRGGBB` or `#RRGGBBAA`. Any you leave out come from `base`.

| Key | Used for |
| --- | --- |
| `frame` | the window frame and title bar |
| `panel` | panels and side bars |
| `panelHeader` | panel headers and tab bars |
| `canvas` | the canvas background |
| `canvasDots` | the dot grid on the canvas |
| `node` | block bodies |
| `nodeHeader` | block title bars |
| `nodeBorder` | block outlines |
| `field` | text boxes and fields |
| `line` | separators |
| `text` | text |
| `textMuted` | secondary text |
| `accent` | selection, links, highlights |
| `action` | action wires and the Run button |
| `statusBar` | the status bar |
| `statusText` | text in the status bar |
| `error` | errors |
| `warning` | warnings |

## pinColors

Pin and wire colours by type id: the basic types `text`, `number`, `boolean`, `date`, `any`, `object`, `list`, and any [pin type](./pin-types) such as `discord.member`.

## shape

| Key | Default | Description |
| --- | --- | --- |
| `nodeRadius` | `9` | Corner radius of blocks. |
| `wireWidth` | `2.6` | Width of wires. |

## motion

| Key | Default | Description |
| --- | --- | --- |
| `speed` | `1.0` | Speed of animations; `0` turns them off. |

Circuit Core's themes (`plugins/xc.core/themes/`) are good starting points.
