// Editor code for this project's plugin: it adds a panel, a command and a
// check to XC Circuit through the global `xc` object. Save this file and the
// plugin reloads by itself. It runs in the editor only, never in the bot.
//
//   xc.command({ id, title, run })   Plugins menu and command palette
//   xc.panel({ id, title, render })  a dockable panel, drawn with ui.*
//   xc.check(fn)                     problems for the Problems panel
//   xc.on("save" | "botEvent", fn)   things happening in the app

const { textValues } = require("./lib/blocks.js");

/** Blocks with "TODO" in one of their texts. */
function todos() {
  const found = [];
  for (const b of xc.blocks()) {
    for (const [field, text] of textValues(b)) {
      if (/\bTODO\b/.test(text)) found.push({ block: b, field, text });
    }
  }
  return found;
}

// What the running bot did this session.
const session = { blocksRun: 0, errors: [] };

xc.on("botEvent", (event) => {
  if (event.type === "started") {
    session.blocksRun = 0;
    session.errors = [];
  }
  if (event.type === "block" && event.phase === "exit") session.blocksRun++;
  if (event.type === "block" && event.error) session.errors.push(event);
});

xc.on("save", () => xc.storage.set("saves", (xc.storage.get("saves") || 0) + 1));

xc.command({
  id: "demo.log_selection",
  title: "Log the selected blocks",
  description: "Writes the names and values of the selected blocks to the Console",
  run() {
    const ws = xc.activeWorkspace();
    const ids = xc.selection();
    if (!ws || ids.length === 0) return xc.warn("Select some blocks first.");
    const nodes = xc.workspace(ws).nodes;
    for (const id of ids) {
      xc.log(`#${id} ${xc.blockName(nodes[id].block)}:`, nodes[id].values || {});
    }
  },
});

xc.check(() =>
  todos().map(({ block, field, text }) => ({
    message: `TODO in ${xc.blockName(block.type)} (${field}): ${text}`,
    workspace: block.workspace,
    block: block.id,
  })),
);

xc.panel({
  id: "notes",
  title: "Project Notes",
  render(ui) {
    const project = xc.project();
    if (!project) return ui.muted("Open a project to see its notes.");

    ui.heading(project.name);
    ui.row((ui) => {
      ui.stat("workspaces", xc.workspaces().length);
      ui.stat("blocks", xc.blocks().length);
      ui.stat("saves here", xc.storage.get("saves") || 0);
    });
    ui.space(6);

    ui.section("Notes", (ui) => {
      ui.input(xc.data.get("notes") || "", (text) => xc.data.set("notes", text), {
        multiline: true,
        placeholder: "Ideas, plans, things to fix… saved with the project",
        key: "notes",
      });
    });

    ui.section("TODOs in blocks", (ui) => {
      const list = todos();
      if (list.length === 0) ui.muted('Write "TODO" in a block\'s text and it shows up here.');
      for (const { block, text } of list) {
        ui.link(`${xc.blockName(block.type)}: ${text}`, () => xc.reveal(block.workspace, block.id), { tooltip: "Show the block" });
      }
    });

    ui.section("Workspaces", (ui) => {
      for (const w of xc.workspaces()) {
        const count = Object.keys(xc.workspace(w.key).nodes || {}).length;
        ui.row((ui) => {
          ui.text(w.name, { bold: w.key === xc.activeWorkspace() });
          ui.text(`${count} blocks`, { muted: true });
        });
      }
    });

    ui.section("Running bot", (ui) => {
      if (!xc.botRunning() && session.blocksRun === 0) return ui.muted("Run the bot to see what it does.");
      ui.text(`${session.blocksRun} blocks run`, { color: xc.botRunning() ? "success" : "muted" });
      for (const e of session.errors.slice(-5)) {
        ui.link(`#${e.block}: ${e.error}`, () => xc.reveal(e.workspace, e.block));
      }
    });

    ui.space(8);
    ui.row((ui) => {
      ui.button("Log the selected blocks", () => xc.run("demo.log_selection"), { primary: true });
      ui.button("Save", () => xc.run("file.save"));
    });
  },
});
