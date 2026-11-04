# Your first editor plugin

Editor plugins change XC Circuit itself: they add panels, commands for the Plugins menu and the command palette, and checks for the Problems panel. They are written in JavaScript and run inside the editor, never in the bot.

In this tutorial you write a *Block Counter*: a panel that shows how many blocks each workspace has, a command that writes the total to the Console, and a check that warns about blocks with an empty text.

## 1. The plugin

Use the project's own plugin from [Your first block](./first-block), or create a new one in `plugins/block.counter/` with this `plugin.toml`:

```toml
id          = "block.counter"
name        = "Block Counter"
version     = "1.0.0"
author      = "You"
description = "Counts blocks."

[engines]
circuit = "^1.0"

[editor]
script = "editor.js"
```

The `[editor]` table names the script. It starts right away: editor plugins run in a sandbox and can only reach the app through the `xc` object, so, unlike native plugins, they don't need your approval.

## 2. A command

Create `editor.js` next to `plugin.toml`:

```js
xc.command({
  id: "counter.total",
  title: "Count all blocks",
  run() {
    xc.log(`${xc.project().name} has ${xc.blocks().length} blocks`);
  },
});
```

Save, then open the **Plugins** menu: *Count all blocks* is there, and in the command palette (**Ctrl+P**) too. Run it and look at the Console.

The script reloads every time you save it, so you can keep XC Circuit open while you work.

## 3. A panel

Add to `editor.js`:

```js
xc.panel({
  id: "counts",
  title: "Block Counter",
  render(ui) {
    if (!xc.project()) return ui.muted("Open a project first.");
    ui.heading("Blocks per workspace");
    for (const w of xc.workspaces()) {
      const count = Object.keys(xc.workspace(w.key).nodes).length;
      ui.row((ui) => {
        ui.link(w.name, () => xc.reveal(w.key, Object.keys(xc.workspace(w.key).nodes)[0]));
        ui.text(`${count} blocks`, { muted: true });
      });
    }
    ui.space();
    ui.button("Count all blocks", () => xc.run("counter.total"), { primary: true });
  },
});
```

Open it from **Plugins › Block Counter panel**. Dock it anywhere, like the Inspector or the Console.

`render(ui)` describes what the panel shows; XC Circuit calls it again whenever the panel is drawn, so it always shows the project as it is now. Buttons and links take a function that runs when they're clicked.

## 4. A check

Checks find problems. Add:

```js
xc.check(() =>
  xc.blocks()
    .filter((b) => b.type === "discord.send_message" && !b.values.text && !b.values.embeds)
    .map((b) => ({
      message: "Send Message has no text",
      workspace: b.workspace,
      block: b.id,
    })),
);
```

Add a Send Message block with an empty text: within a second the Problems panel lists it, and clicking the entry selects the block.

## 5. Remember something

A panel can keep its own state in variables of the script, and the plugin can store small values that survive restarts:

```js
let clicks = xc.storage.get("clicks") ?? 0;

xc.panel({
  id: "clicker",
  title: "Clicker",
  render(ui) {
    ui.button(`Clicked ${clicks} times`, () => xc.storage.set("clicks", ++clicks));
  },
});
```

## What you learned

- An editor plugin is a script named in `[editor]` of `plugin.toml`.
- `xc.command`, `xc.panel` and `xc.check` add commands, panels and checks.
- Panels are described with `ui.*` and redrawn when shown or clicked.

All of `xc` is in [Editor plugins (xc)](/api/editor), every panel element in [Panels (ui)](/api/panels). DemoBot's `plugins/demo.custom/editor.js` is a bigger example.
