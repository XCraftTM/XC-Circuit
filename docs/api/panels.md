# Panels (ui)

A panel's `render(ui)` describes what it shows by calling `ui` methods in order. XC Circuit draws the result with the app's own look and theme.

```js
xc.panel({
  id: "stats",
  title: "Stats",
  render(ui) {
    ui.heading("Project");
    ui.row((ui) => {
      ui.stat("workspaces", xc.workspaces().length);
      ui.stat("blocks", xc.blocks().length);
    });
    ui.section("Actions", (ui) => {
      ui.button("Save", () => xc.run("file.save"), { primary: true });
    });
  },
});
```

## How panels update

- `render` runs when the panel is drawn: when it's opened, after a click or a change in it, and at most four times a second otherwise (about once a second while nothing else happens).
- Handlers (the functions given to buttons, links, check boxes and inputs) run when the person uses that element; the panel is drawn again right after.
- Panels are **controlled**: an input shows the value `render` gives it. Store what the person types (in a variable, `xc.storage` or `xc.data`) and give it back on the next render, or it will be undone.

## Elements

Every method returns `ui`, so calls can be chained.

### Text

| Method | Shows |
| --- | --- |
| `ui.heading(text)` | A bold title. |
| `ui.text(text, options?)` | A paragraph; wraps. |
| `ui.muted(text)` | Grey text, for hints. |
| `ui.stat(label, value)` | A big value with a small label under it. Put several in a `row`. |

`ui.text` options:

| Option | Description |
| --- | --- |
| `muted` | grey |
| `bold` | strong |
| `mono` | monospaced |
| `size` | font size in points (default 12.5) |
| `color` | `"accent"`, `"success"`, `"warning"`, `"error"`, `"muted"` or `"#RRGGBB"` |

### Controls

| Method | Shows |
| --- | --- |
| `ui.button(label, onClick, options?)` | A button. Options: `primary` (filled with the accent), `tooltip`. Without `onClick` it's disabled. |
| `ui.link(label, onClick, options?)` | A link-style button. Options: `tooltip`. |
| `ui.checkbox(label, checked, onChange)` | A check box. `onChange(checked)`. |
| `ui.input(value, onChange, options?)` | A text box. `onChange(text)` on every change. Options: `placeholder`, `multiline` (a box of several lines), `key` (keeps the text box's focus when the panel's layout around it changes). |
| `ui.progress(fraction, label?)` | A progress bar, `fraction` from 0 to 1. |

### Layout

| Method | Shows |
| --- | --- |
| `ui.row(fn)` | What `fn(ui)` adds, side by side (wrapping when narrow). |
| `ui.section(title, fn, options?)` | A section with a title that folds open and closed. Options: `open: false` starts it closed. |
| `ui.separator()` | A line. |
| `ui.space(points = 8)` | Empty space. |

`row` and `section` get their own `ui`; use it for what goes inside:

```js
ui.section("Workspaces", (ui) => {
  for (const w of xc.workspaces()) {
    ui.row((ui) => {
      ui.text(w.name, { bold: w.key === xc.activeWorkspace() });
      ui.link("Open", () => xc.reveal(w.key, Object.keys(xc.workspace(w.key).nodes)[0]));
    });
  }
});
```

## Errors

If `render` throws, the panel shows the error with its file and line, above what it showed last. Fix the script and save it: the plugin reloads by itself.
