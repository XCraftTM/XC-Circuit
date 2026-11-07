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

// ---- Reading ----------------------------------------------------------------

xc.tool({
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

xc.tool({
  name: "get_workspace",
  description: "A workspace's blocks (id, type, name, position, values, fields made pins) and wires (\"1.next > 2.in\": block 1's flow runs block 2; \"3.result > 4.text\": a value into a field).",
  input: schema({ workspace: workspaceArg }, ["workspace"]),
  run({ workspace }) {
    needProject();
    const { key, ws } = load(workspace);
    const blocks = Object.entries(ws.nodes || {}).map(([id, n]) => ({ id, type: String(n.block).split("@")[0], name: xc.blockName(n.block), at: n.at, values: n.values || {}, pins: n.pins || [] }));
    return { key, name: ws.name, folder: ws.folder || null, turnedOff: !!ws.disabled, blocks, wires: ws.links || [] };
  },
});

xc.tool({
  name: "find_block_types",
  description: "Searches the installed block types by words (\"send message\", \"timer\", \"database\"). Without a query, lists them all. Returns type, name, category, kind (event, action, value) and description.",
  input: schema({ query: { type: "string", description: "Words to look for; empty for all." } }),
  run({ query }) {
    const found = xc.blockTypes(query || "");
    const max = 80;
    return found.length > max ? { blockTypes: found.slice(0, max), more: `${found.length - max} more: search with more words.` } : { blockTypes: found };
  },
});

xc.tool({
  name: "get_block_type",
  description: "A block type's fields (key, type, default, choices, whether it can take a wire), outputs and flows. Read it before adding or setting a block.",
  input: schema({ type: { type: "string", description: "The block type, such as discord.send_message." } }, ["type"]),
  run({ type }) {
    const info = xc.blockType(type);
    if (!info) throw new Error(`No block type "${type}" is installed. Search with find_block_types.`);
    return info;
  },
});

xc.tool({
  name: "get_block",
  description: "One block as it is now: its field values and every pin it has (a block's settings can add pins), with their types and the wires on them.",
  input: schema({ workspace: workspaceArg, block: blockArg }, ["workspace", "block"]),
  run({ workspace, block }) {
    needProject();
    return xc.blockPins(resolve(workspace), String(block));
  },
});

xc.tool({
  name: "get_problems",
  description: "What the Problems panel lists: missing settings, unconnected required inputs, plugin problems. Each with the workspace and block it's about.",
  input: schema({}),
  run() {
    needProject();
    const list = xc.problems();
    return list.length ? list : "No problems.";
  },
});

xc.tool({
  name: "get_console",
  description: "The Console's last lines: the running bot's log and errors (with the block that wrote them), and the editor's messages.",
  input: schema({ lines: { type: "integer", description: "How many lines (50 when not given, at most 1000).", minimum: 1, maximum: 1000 } }),
  run({ lines }) {
    return xc.console(lines || 50);
  },
});

xc.tool({
  name: "get_slash_commands",
  description: "The project's slash commands as the Slash Commands editor keeps them: name (with spaces for subcommands), description, options, who can use them and where.",
  input: schema({}),
  run() {
    needProject();
    return xc.data.get("slashCommands") || [];
  },
});

// ---- Changing ---------------------------------------------------------------

xc.tool({
  name: "add_block",
  description: "Adds a block to a workspace. Without x and y it goes right of the block given as `after` (or of the rightmost block). Returns the new block with its pins. Wire it up with connect.",
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
    const id = xc.addBlock(key, type, { x: at[0], y: at[1], values: values || {} });
    return xc.blockPins(key, id);
  },
});

xc.tool({
  name: "set_block_values",
  description: "Sets fields of a block: { fieldKey: value }. null puts a field back to its default. Text fields can hold ${…} with JavaScript.",
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
      else node.values[k] = v;
    }
    xc.setWorkspace(key, ws);
    return xc.blockPins(key, id);
  },
});

xc.tool({
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

xc.tool({
  name: "connect",
  description: "Draws a wire, checked like one drawn on the canvas. Flow: from a block's flow (\"next\", \"failed\", …) to another block's \"in\". Value: from an output to a field (it becomes a pin). Pins are written block.pin, such as 1.next and 2.in.",
  input: schema({ workspace: workspaceArg, from: { type: "string", description: "block.pin, such as 1.next or 3.result" }, to: { type: "string", description: "block.pin, such as 2.in or 4.text" } }, ["workspace", "from", "to"]),
  run({ workspace, from, to }) {
    needProject();
    xc.connect(resolve(workspace), from, to);
    return `Connected ${from} > ${to}.`;
  },
});

xc.tool({
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

xc.tool({
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

xc.tool({
  name: "create_workspace",
  description: "Makes a new, empty workspace (in a sidebar folder, when given) and opens it. Returns its key.",
  input: schema({ name: str, folder: { type: "string", description: "A sidebar folder to put it in." } }, ["name"]),
  run({ name, folder }) {
    needProject();
    const key = xc.addWorkspace(name, { folder });
    return { key, name };
  },
});

xc.tool({
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
    return `Saved ${commands.length} slash commands.`;
  },
});

xc.tool({
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

xc.tool({
  name: "save_project",
  description: "Saves the project's files (the editor saves by itself too, before the bot runs).",
  input: schema({}),
  run() {
    needProject();
    xc.run("file.save");
    return "Saving.";
  },
});

// ---- Running ----------------------------------------------------------------

xc.tool({
  name: "run_bot",
  description: "Starts (or restarts) the bot with the project as it is. Check get_console a few seconds later to see it log in and what it does.",
  input: schema({}),
  run() {
    needProject();
    xc.run("bot.run");
    return "Starting the bot. Its log shows in get_console.";
  },
});

xc.tool({
  name: "stop_bot",
  description: "Stops the running bot.",
  input: schema({}),
  run() {
    if (!xc.botRunning()) return "The bot isn't running.";
    xc.run("bot.stop");
    return "Stopping the bot.";
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
