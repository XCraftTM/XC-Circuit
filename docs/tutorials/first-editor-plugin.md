# Your first editor plugin

Editor plugins add things to XC Circuit itself: panels, commands, checks, even a [desk pet](https://github.com/XCraftTM/XC-Circuit/tree/main/examples/plugins/desk-pet). They're JavaScript too.

We'll make a **Block Counter**: a command and a panel.

## 1. The plugin

In your bot's folder: `plugins/block.counter/plugin.toml`

```toml
id      = "block.counter"
name    = "Block Counter"
version = "1.0.0"

[editor]
script = "editor.js"
```

## 2. The script

`plugins/block.counter/editor.js`:

```js
xc.command({
  id: "counter.total",
  title: "Count all blocks",
  run() {
    xc.log(`${xc.project().name} has ${xc.blocks().length} blocks`);
  },
});

xc.panel({
  id: "counts",
  title: "Block Counter",
  render(ui) {
    ui.heading("Blocks per workspace");
    for (const w of xc.workspaces()) {
      const count = Object.keys(xc.workspace(w.key).nodes).length;
      ui.row((ui) => {
        ui.text(w.name);
        ui.text(`${count} blocks`, { muted: true });
      });
    }
    ui.button("Count all blocks", () => xc.run("counter.total"), { primary: true });
  },
});
```

Save. It loads right away, and again every time you save.

## 3. Try it

Open the **Plugins** menu:

![The Plugins menu](/screens/plugins-menu.png)

1. Your command: writes to the Console.
2. Your panel: opens it.

![The panel](/screens/block-counter-panel.png)

1. Dock it anywhere, like any panel.

## What else `xc` can do

| | |
| --- | --- |
| `xc.check(fn)` | problems for the Problems panel |
| `xc.overlay({…})` | something floating over the editor (the desk pet) |
| `xc.every(ms, fn)` | timers |
| `xc.playSound(file)` | sounds |
| `xc.addBlock(…)`, `xc.select(…)` | change the canvas |
| `xc.on("save" \| "botEvent" \| "selection", fn)` | react to the app |

All of it: [Editor plugins (xc)](/api/editor) and [Panels (ui)](/api/panels).
