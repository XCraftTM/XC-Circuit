// AI agents (MCP): the tools AI agents such as Claude use through XC Circuit's
// MCP server (`xc-circuit --mcp`), and a panel that adds the server to AI
// clients. Every change goes through the editor, so it shows at once and can
// be undone. The bot's token is never given out.

const t = (text) => xc.t(text);

// ---- Helpers ----------------------------------------------------------------

/** A workspace by key or name. */
function resolve(workspace) {
  const all = xc.workspaces();
  const wanted = String(workspace || "").trim();
  const found = all.find((w) => w.key === wanted) || all.find((w) => w.name.toLowerCase() === wanted.toLowerCase());
  if (!found) {
    throw new Error(`No workspace "${wanted}". There are: ${all.map((w) => `${w.key} (${w.name})`).join(", ") || "none"}.`);
  }
  return found.key;
}

function load(workspace) {
  const key = resolve(workspace);
  return { key, ws: xc.workspace(key) };
}

function needProject() {
  if (!xc.project()) throw new Error("No project is open in XC Circuit. Open or create one there first.");
}

function needBlock(ws, key, block) {
  const id = String(block);
  if (!ws.nodes || !ws.nodes[id]) throw new Error(`Workspace ${key} has no block "${id}". Its blocks: ${Object.keys(ws.nodes || {}).join(", ") || "none"}.`);
  return id;
}

const str = { type: "string" };
const workspaceArg = { type: "string", description: "The workspace's key or name (from get_project)." };
const blockArg = { type: "string", description: "The block's id in the workspace (from get_workspace)." };
const schema = (properties, required = []) => ({ type: "object", properties, required });

// Every tool's run(), for run_script: tools.add_block({ … }) does what the tool does.
const runs = {};
function tool(def) {
  runs[def.name] = def.run;
  xc.tool(def);
}

// ---- Reading ----------------------------------------------------------------

tool({
  name: "get_project",
  description: "The open project: its name, workspaces (key, name, folder, block count, whether turned off), how many problems it has, its slash commands and whether the bot is running. Start here.",
  input: schema({}),
  run() {
    const project = xc.project();
    if (!project) return "No project is open in XC Circuit.";
    const workspaces = xc.workspaces().map((w) => {
      const ws = xc.workspace(w.key) || {};
      return { key: w.key, name: w.name, folder: ws.folder || null, blocks: Object.keys(ws.nodes || {}).length, turnedOff: !!ws.disabled };
    });
    const commands = xc.data.get("slashCommands") || [];
    return { name: project.name, workspaces, problems: xc.problems().length, slashCommands: commands.map((c) => c.name), botRunning: xc.botRunning() };
  },
});

tool({
  name: "get_workspace",
  description: "A workspace's blocks (id, type, name, position, values, fields made pins), wires (\"1.next > 2.in\": block 1's flow runs block 2; \"3.result > 4.text\": a value into a field) and notes (framed areas with a title and text).",
  input: schema({ workspace: workspaceArg }, ["workspace"]),
  run({ workspace }) {
    needProject();
    const { key, ws } = load(workspace);
    const blocks = Object.entries(ws.nodes || {}).map(([id, n]) => ({ id, type: String(n.block).split("@")[0], name: xc.blockName(n.block), at: n.at, values: n.values || {}, pins: n.pins || [] }));
    const notes = Object.entries(ws.notes || {}).map(([id, n]) => ({ id, title: n.title, text: n.text || "", color: n.color || null, x: n.rect[0], y: n.rect[1], width: n.rect[2], height: n.rect[3] }));
    return { key, name: ws.name, folder: ws.folder || null, turnedOff: !!ws.disabled, blocks, wires: ws.links || [], notes };
  },
});

tool({
  name: "find_block_types",
  description: "Searches the installed block types by words (\"send message\", \"timer\", \"database\"). Without a query, lists them all. Returns type, name, category, kind (event, action, value) and description.",
  input: schema({ query: { type: "string", description: "Words to look for; empty for all." } }),
  run({ query }) {
    const found = xc.blockTypes(query || "");
    const max = 80;
    return found.length > max ? { blockTypes: found.slice(0, max), more: `${found.length - max} more: search with more words.` } : { blockTypes: found };
  },
});

tool({
  name: "get_block_type",
  description: "A block type's fields (key, type, default, choices, whether it can take a wire), outputs and flows. Read it before adding or setting a block. Fields a builder edits (embed, layout, canvas) say their value's `format`.",
  input: schema({ type: { type: "string", description: "The block type, such as discord.send_message." } }, ["type"]),
  run({ type }) {
    const info = xc.blockType(type);
    if (!info) throw new Error(`No block type "${type}" is installed. Search with find_block_types.`);
    return info;
  },
});

tool({
  name: "get_block",
  description: "One block as it is now: its field values and every pin it has (a block's settings can add pins), with their types and the wires on them.",
  input: schema({ workspace: workspaceArg, block: blockArg }, ["workspace", "block"]),
  run({ workspace, block }) {
    needProject();
    return xc.blockPins(resolve(workspace), String(block));
  },
});

tool({
  name: "get_problems",
  description: "What the Problems panel lists: missing settings, unconnected required inputs, plugin problems. Each with the workspace and block it's about.",
  input: schema({}),
  run() {
    needProject();
    const list = xc.problems();
    return list.length ? list : "No problems.";
  },
});

tool({
  name: "get_console",
  description: "The Console's last lines: the running bot's log and errors (with the block that wrote them), and the editor's messages.",
  input: schema({ lines: { type: "integer", description: "How many lines (50 when not given, at most 1000).", minimum: 1, maximum: 1000 } }),
  run({ lines }) {
    return xc.console(lines || 50);
  },
});

tool({
  name: "get_slash_commands",
  description: "The project's slash commands as the Slash Commands editor keeps them: name (with spaces for subcommands), description, options, who can use them and where.",
  input: schema({}),
  run() {
    needProject();
    return xc.data.get("slashCommands") || [];
  },
});

// ---- Changing ---------------------------------------------------------------

tool({
  name: "add_block",
  description: "Adds a block to a workspace. Without x and y it goes right of the block given as `after` (or of the rightmost block). Returns the new block with its pins. Wire it up with connect. Embeds and layouts can be given as Discord's own JSON.",
  input: schema(
    {
      workspace: workspaceArg,
      type: { type: "string", description: "The block type, such as core.log (see find_block_types)." },
      values: { type: "object", description: "Field values over its defaults: { fieldKey: value }." },
      after: { type: "string", description: "A block id to place it next to." },
      x: { type: "number" },
      y: { type: "number" },
    },
    ["workspace", "type"],
  ),
  run({ workspace, type, values, after, x, y }) {
    needProject();
    const { key, ws } = load(workspace);
    let at = typeof x === "number" && typeof y === "number" ? [x, y] : null;
    if (!at) {
      const nodes = Object.entries(ws.nodes || {});
      const anchor = after && ws.nodes[after] ? ws.nodes[after] : null;
      if (anchor) at = [anchor.at[0] + 400, anchor.at[1]];
      else if (nodes.length) at = [Math.max(...nodes.map(([, n]) => n.at[0])) + 400, Math.min(...nodes.map(([, n]) => n.at[1]))];
      else at = [40, 60];
    }
    const given = {};
    for (const [k, v] of Object.entries(values || {})) given[k] = xc.fieldValue(type, k, v);
    const id = xc.addBlock(key, type, { x: at[0], y: at[1], values: given });
    return xc.blockPins(key, id);
  },
});

tool({
  name: "set_block_values",
  description: "Sets fields of a block: { fieldKey: value }. null puts a field back to its default. Text fields can hold ${…} with JavaScript. Embed, layout and canvas fields (what the builders edit) take the `format` get_block_type gives; embeds and layouts also Discord's own JSON.",
  input: schema({ workspace: workspaceArg, block: blockArg, values: { type: "object" } }, ["workspace", "block", "values"]),
  run({ workspace, block, values }) {
    needProject();
    const { key, ws } = load(workspace);
    const id = needBlock(ws, key, block);
    const known = xc.blockPins(key, id).fields.map((f) => f.key);
    const node = ws.nodes[id];
    node.values = node.values || {};
    for (const [k, v] of Object.entries(values || {})) {
      if (!known.includes(k) && !(k in node.values)) throw new Error(`Block ${id} has no field "${k}". Its fields: ${known.join(", ")}.`);
      if (v === null) delete node.values[k];
      else node.values[k] = xc.fieldValue(node.block, k, v);
    }
    xc.setWorkspace(key, ws);
    return xc.blockPins(key, id);
  },
});

tool({
  name: "move_block",
  description: "Moves a block on the canvas.",
  input: schema({ workspace: workspaceArg, block: blockArg, x: { type: "number" }, y: { type: "number" } }, ["workspace", "block", "x", "y"]),
  run({ workspace, block, x, y }) {
    needProject();
    const { key, ws } = load(workspace);
    const id = needBlock(ws, key, block);
    ws.nodes[id].at = [Number(x) || 0, Number(y) || 0];
    xc.setWorkspace(key, ws);
    return `Moved block ${id}.`;
  },
});

tool({
  name: "connect",
  description: "Draws a wire, checked like one drawn on the canvas. Flow: from a block's flow (\"next\", \"failed\", …) to another block's \"in\". Value: from an output to a field (it becomes a pin). Pins are written block.pin, such as 1.next and 2.in.",
  input: schema({ workspace: workspaceArg, from: { type: "string", description: "block.pin, such as 1.next or 3.result" }, to: { type: "string", description: "block.pin, such as 2.in or 4.text" } }, ["workspace", "from", "to"]),
  run({ workspace, from, to }) {
    needProject();
    xc.connect(resolve(workspace), from, to);
    return `Connected ${from} > ${to}.`;
  },
});

tool({
  name: "disconnect",
  description: "Takes a wire out: the same from and to as connect.",
  input: schema({ workspace: workspaceArg, from: str, to: str }, ["workspace", "from", "to"]),
  run({ workspace, from, to }) {
    needProject();
    const { key, ws } = load(workspace);
    const wire = `${String(from).trim()} > ${String(to).trim()}`;
    const before = (ws.links || []).length;
    ws.links = (ws.links || []).filter((l) => l !== wire);
    if (ws.links.length === before) throw new Error(`There's no wire ${wire}. The wires: ${(ws.links || []).join(", ") || "none"}.`);
    xc.setWorkspace(key, ws);
    return `Removed ${wire}.`;
  },
});

tool({
  name: "delete_blocks",
  description: "Deletes blocks from a workspace, with their wires. It can be undone in the editor.",
  input: schema({ workspace: workspaceArg, blocks: { type: "array", items: str, description: "Block ids." } }, ["workspace", "blocks"]),
  run({ workspace, blocks }) {
    needProject();
    const { key, ws } = load(workspace);
    const ids = (blocks || []).map((b) => needBlock(ws, key, b));
    for (const id of ids) delete ws.nodes[id];
    const gone = (pin) => ids.includes(pin.trim().split(".")[0]);
    ws.links = (ws.links || []).filter((l) => !l.split(">").some(gone));
    xc.setWorkspace(key, ws);
    return `Deleted ${ids.length} block${ids.length === 1 ? "" : "s"}.`;
  },
});

tool({
  name: "create_workspace",
  description: "Makes a new, empty workspace (in a sidebar folder, when given) and opens it. Returns its key.",
  input: schema({ name: str, folder: { type: "string", description: "A sidebar folder to put it in." } }, ["name"]),
  run({ name, folder }) {
    needProject();
    const key = xc.addWorkspace(name, { folder });
    return { key, name };
  },
});

tool({
  name: "set_slash_commands",
  description: "Replaces the project's slash commands (the list get_slash_commands gives, changed). The bot registers them with Discord when it starts. Read them first: entries you leave out are deleted.",
  input: schema({ commands: { type: "array", items: { type: "object" } } }, ["commands"]),
  run({ commands }) {
    needProject();
    if (!Array.isArray(commands)) throw new Error("commands must be a list");
    for (const c of commands) {
      if (!c || typeof c.name !== "string" || !c.name.trim()) throw new Error("every command needs a name");
    }
    xc.data.set("slashCommands", commands);
    return `Saved ${commands.length} slash command${commands.length === 1 ? "" : "s"}.`;
  },
});

tool({
  name: "show_in_editor",
  description: "Opens a workspace in the editor and, with a block, selects it and centres the canvas on it: shows the user what you mean.",
  input: schema({ workspace: workspaceArg, block: blockArg }, ["workspace"]),
  run({ workspace, block }) {
    needProject();
    const { key, ws } = load(workspace);
    const id = block ? needBlock(ws, key, block) : Object.keys(ws.nodes || {})[0];
    if (id) xc.reveal(key, id);
    return "Shown.";
  },
});

tool({
  name: "save_project",
  description: "Saves the project's files (the editor saves by itself too, before the bot runs).",
  input: schema({}),
  run() {
    needProject();
    xc.run("file.save");
    return "Saving.";
  },
});

// ---- Notes ------------------------------------------------------------------

const noteArg = { type: "string", description: "The note's id (from get_workspace), such as n1." };
const noteFields = {
  title: { type: "string" },
  text: { type: "string", description: "Shown under the title." },
  color: { type: "string", description: "#RRGGBB; the theme's accent when not given." },
  around: { type: "array", items: str, description: "Block ids: the note frames them (instead of x, y, width, height)." },
  x: { type: "number" },
  y: { type: "number" },
  width: { type: "number" },
  height: { type: "number" },
};

/** Roughly where a block is on the canvas: [x, y, width, height]. */
function blockArea(key, ws, id) {
  const n = ws.nodes[id];
  const pins = xc.blockPins(key, id);
  const rows = pins.fields.filter((f) => !f.advanced).length + pins.outputs.length + pins.flows.filter((f) => f.key !== "next").length;
  return [n.at[0], n.at[1], n.width || 260, 44 + rows * 26];
}

/** A note's rectangle from x, y, width, height or the blocks it goes around. */
function noteRect(key, ws, o, before) {
  if (Array.isArray(o.around) && o.around.length) {
    const areas = o.around.map((b) => blockArea(key, ws, needBlock(ws, key, b)));
    const left = Math.min(...areas.map((a) => a[0])) - 30;
    const top = Math.min(...areas.map((a) => a[1])) - 56;
    const right = Math.max(...areas.map((a) => a[0] + a[2])) + 30;
    const bottom = Math.max(...areas.map((a) => a[1] + a[3])) + 30;
    return [left, top, right - left, bottom - top];
  }
  const r = before || [40, 40, 420, 260];
  return [o.x ?? r[0], o.y ?? r[1], o.width ?? r[2], o.height ?? r[3]].map(Number);
}

tool({
  name: "add_note",
  description: "Adds a note to a workspace: a framed area with a title and text, to explain or group blocks. Give `around` (block ids) to frame those blocks, or x, y, width and height. Returns its id.",
  input: schema({ workspace: workspaceArg, ...noteFields }, ["workspace", "title"]),
  run(o) {
    needProject();
    const { key, ws } = load(o.workspace);
    ws.notes = ws.notes || {};
    let n = 1;
    while (ws.notes[`n${n}`]) n += 1;
    const id = `n${n}`;
    const note = { title: String(o.title || ""), rect: noteRect(key, ws, o, null) };
    if (o.text) note.text = String(o.text);
    if (o.color) note.color = String(o.color);
    ws.notes[id] = note;
    xc.setWorkspace(key, ws);
    return { id, ...note };
  },
});

tool({
  name: "set_note",
  description: "Changes a note: its title, text, colour, or where it is (x, y, width, height, or `around` blocks). What isn't given stays.",
  input: schema({ workspace: workspaceArg, note: noteArg, ...noteFields }, ["workspace", "note"]),
  run(o) {
    needProject();
    const { key, ws } = load(o.workspace);
    const note = (ws.notes || {})[o.note];
    if (!note) throw new Error(`Workspace ${key} has no note "${o.note}". Its notes: ${Object.keys(ws.notes || {}).join(", ") || "none"}.`);
    if (o.title !== undefined) note.title = String(o.title);
    if (o.text !== undefined) note.text = String(o.text);
    if (o.color !== undefined) note.color = o.color ? String(o.color) : undefined;
    if (o.around || ["x", "y", "width", "height"].some((k) => o[k] !== undefined)) note.rect = noteRect(key, ws, o, note.rect);
    xc.setWorkspace(key, ws);
    return { id: o.note, ...note };
  },
});

tool({
  name: "delete_note",
  description: "Deletes a note. The blocks inside it stay.",
  input: schema({ workspace: workspaceArg, note: noteArg }, ["workspace", "note"]),
  run({ workspace, note }) {
    needProject();
    const { key, ws } = load(workspace);
    if (!(ws.notes || {})[note]) throw new Error(`Workspace ${key} has no note "${note}".`);
    delete ws.notes[note];
    xc.setWorkspace(key, ws);
    return `Deleted note ${note}.`;
  },
});

// ---- Builders ---------------------------------------------------------------

tool({
  name: "open_builder",
  description: "Opens the editor's Embed, Layout or Canvas Builder on a block's field (or Slash Commands, for a Slash Command block), so the user sees it there. To change what's in it, use set_block_values.",
  input: schema({ workspace: workspaceArg, block: blockArg, field: { type: "string", description: "The field; the block's first one a builder edits when not given." } }, ["workspace", "block"]),
  run({ workspace, block, field }) {
    needProject();
    const builder = xc.openBuilder(resolve(workspace), String(block), field || "");
    return `Opened the ${builder}.`;
  },
});

// ---- Running ----------------------------------------------------------------

tool({
  name: "run_bot",
  description: "Starts (or restarts) the bot with the project as it is. Check get_console a few seconds later to see it log in and what it does.",
  input: schema({}),
  run() {
    needProject();
    xc.run("bot.run");
    return "Starting the bot. Its log shows in get_console.";
  },
});

tool({
  name: "stop_bot",
  description: "Stops the running bot.",
  input: schema({}),
  run() {
    if (!xc.botRunning()) return "The bot isn't running.";
    xc.run("bot.stop");
    return "Stopping the bot.";
  },
});

// ---- Many things at once ------------------------------------------------------

xc.tool({
  name: "run_script",
  description:
    "Runs JavaScript in XC Circuit to do many things in one call: build a whole flow, change many blocks, look things up. " +
    "The script gets `tools` (every other tool as a function taking the same arguments and returning the same answer, such as " +
    "tools.add_block({ workspace, type, values }) and tools.connect({ workspace, from, to })), `xc` (XC Circuit's plugin API: " +
    "xc.workspace(key), xc.setWorkspace(key, ws), xc.blockTypes(query) …) and `print(...)`. It returns what was printed and the " +
    "script's return value. A thrown error stops it and says where it got to; changes made before stay (undo them in the editor). " +
    "It runs for at most 5 seconds, synchronously, and can't reach files or the network.",
  input: schema({ code: { type: "string", description: "The body of a function: statements, and `return` for a result." } }, ["code"]),
  run({ code }) {
    const output = [];
    const print = (...parts) => output.push(parts.map((p) => (typeof p === "string" ? p : JSON.stringify(p))).join(" "));
    const tools = Object.freeze(Object.fromEntries(Object.entries(runs).map(([name, run]) => [name, (args) => run(args || {})])));
    let fn;
    try {
      fn = new Function("xc", "tools", "print", `"use strict";\n${code}`);
    } catch (e) {
      throw new Error(`The script doesn't parse: ${e.message}`);
    }
    try {
      const result = fn(xc, tools, print);
      return { output, result: result === undefined ? null : result };
    } catch (e) {
      const said = output.length ? `\nPrinted before it stopped:\n${output.join("\n")}` : "";
      throw new Error(`The script stopped: ${e && e.message !== undefined ? e.message : String(e)}${said}`);
    }
  },
});

// ---- The panel: adding the server to AI clients ------------------------------

let clients = null;
let said = "";

function refresh() {
  clients = xc.agents.clients();
}

const STATE = {
  added: "Added",
  outdated: "Added, but for another copy of XC Circuit",
  missing: "Not added",
  absent: "Not installed on this computer",
};

xc.panel({
  id: "agents",
  title: "AI agents",
  render(ui) {
    if (!clients) refresh();
    const link = xc.agents.link() || {};
    ui.heading(t("AI agents (MCP)"));
    ui.muted(t("AI agents such as Claude can read your project, add, change and connect blocks, see problems and the Console, and start or stop the bot. Every change shows here at once and can be undone. They never see the bot's token."));
    ui.space(6);
    const status = !link.listening ? t("Not listening") : link.agents ? t("Listening · {n} connected").replace("{n}", link.agents) : t("Listening · no agent connected");
    ui.row((ui) => {
      ui.text(t("Server"), { bold: true });
      ui.text(status, { color: link.agents ? "success" : null, muted: !link.agents });
    });
    ui.space(8);

    ui.section(t("Add to an AI client"), (ui) => {
      for (const c of clients) {
        ui.row((ui) => {
          ui.text(c.name, { bold: true });
          ui.text(t(STATE[c.state] || c.state), { muted: c.state !== "added", color: c.state === "added" ? "success" : c.state === "outdated" ? "warning" : null });
        });
        ui.row((ui) => {
          if (c.state === "added") {
            ui.button(t("Remove"), () => act(c.id, true), { tooltip: t("Take XC Circuit out of its settings") });
          } else {
            ui.button(c.state === "outdated" ? t("Update") : t("Add"), () => act(c.id, false), { primary: c.state !== "absent", tooltip: c.detail });
          }
        });
        ui.space(4);
      }
      ui.link(t("Check again"), refresh);
    });

    ui.section(t("Other MCP clients"), (ui) => {
      ui.muted(t("Paste this into the client's MCP settings: it starts XC Circuit's server."));
      ui.button(t("Copy settings"), () => {
        xc.copyText(xc.agents.snippet());
        said = t("Copied.");
      });
    });
    if (said) {
      ui.space(6);
      ui.note(said);
    }
    ui.space(6);
    ui.muted(t("The agent's tools work while XC Circuit is open. Turn this plugin off or remove it under Plugins to stop them."));
  },
});

function act(client, remove) {
  try {
    said = remove ? xc.agents.remove(client) : xc.agents.install(client);
  } catch (e) {
    said = e.message;
  }
  refresh();
}
