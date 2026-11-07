// The `xc` object editor plugins use, and the calls XC Circuit makes into a
// plugin's script (`__xc`). Everything crosses to the editor as JSON text.
"use strict";
(() => {
  const host = (method, ...args) => {
    const r = JSON.parse(__host(method, JSON.stringify(args)));
    if (r.error !== undefined) throw new Error(r.error);
    return r.ok;
  };
  // The script's own part in the editor: timers, overlays, sounds.
  const own = (method, ...args) => {
    const r = JSON.parse(__self(method, JSON.stringify(args)));
    if (r.error !== undefined) throw new Error(r.error);
    return r.ok;
  };
  const text = (v) => (v === undefined || v === null ? "" : typeof v === "string" ? v : typeof v === "object" ? JSON.stringify(v) : String(v));
  const need = (ok, message) => {
    if (!ok) throw new TypeError(message);
  };
  // The error and where it happened in the plugin's files (not in this prelude).
  const fail = (e) => {
    if (!e || e.message === undefined) return String(e);
    const where = String(e.stack || "")
      .split("\n")
      .filter((line) => line.trim() && !line.includes("eval_script"))
      .join("\n");
    return where ? `${e.message}\n${where}` : e.message;
  };

  const commands = new Map();
  const panels = new Map();
  const tools = new Map();
  const checks = [];
  const overlays = new Map();
  const timers = new Map();
  let nextTimer = 1;
  const listeners = { save: [], botEvent: [], selection: [] };
  // Each panel's click and change handlers from its last render, by number.
  const handlers = new Map();

  const log = (level) => (...parts) => host("log", level, parts.map(text).join(" "));

  const xc = {
    /** Adds a command to the Plugins menu and the command palette. */
    command(def) {
      need(def && typeof def.id === "string" && def.id, "xc.command({ id, title, run }) needs an id");
      need(typeof def.run === "function", `the command "${def.id}" needs a run() function`);
      need(!commands.has(def.id), `the command "${def.id}" is added twice`);
      commands.set(def.id, { id: def.id, title: text(def.title || def.id), description: text(def.description), run: def.run });
    },
    /** Adds a dockable panel; render(ui) is called again whenever it's shown. */
    panel(def) {
      need(def && typeof def.id === "string" && def.id, "xc.panel({ id, title, render }) needs an id");
      need(typeof def.render === "function", `the panel "${def.id}" needs a render(ui) function`);
      need(!panels.has(def.id), `the panel "${def.id}" is added twice`);
      panels.set(def.id, { id: def.id, title: text(def.title || def.id), render: def.render });
    },
    /** Adds a check: a function returning problems ({ message, workspace, block } or text) for the Problems panel. */
    check(fn) {
      need(typeof fn === "function", "xc.check(fn) needs a function");
      checks.push(fn);
    },
    /**
     * Something AI agents can do through XC Circuit's MCP server: { name,
     * description, input (a JSON Schema for its arguments), run(args) }.
     * run returns the answer: text, or any value (sent as JSON).
     */
    tool(def) {
      need(def && typeof def.name === "string" && /^[a-zA-Z0-9_-]{1,64}$/.test(def.name), "xc.tool({ name, description, input, run }) needs a name of letters, digits, _ and -");
      need(typeof def.run === "function", `the tool "${def.name}" needs a run(args) function`);
      need(!tools.has(def.name), `the tool "${def.name}" is added twice`);
      tools.set(def.name, { name: def.name, description: text(def.description), input: def.input || { type: "object", properties: {} }, run: def.run });
    },
    /** Floats something over the whole editor: render(ui) like a panel, dragged around, onClick when clicked. */
    overlay(def) {
      need(def && typeof def.id === "string" && def.id, "xc.overlay({ id, width, height, render }) needs an id");
      need(typeof def.render === "function", `the overlay "${def.id}" needs a render(ui) function`);
      need(!overlays.has(def.id), `the overlay "${def.id}" is added twice`);
      overlays.set(def.id, {
        id: def.id,
        x: Number(def.x ?? -24),
        y: Number(def.y ?? -60),
        width: Number(def.width) || 120,
        height: Number(def.height) || 120,
        frame: !!def.frame,
        draggable: def.draggable !== false,
        render: def.render,
        onClick: typeof def.onClick === "function" ? def.onClick : null,
      });
    },
    /** Moves an overlay so its top-left corner is at x, y (in the window). */
    moveOverlay: (id, x, y) => own("overlayMove", String(id), Number(x) || 0, Number(y) || 0),
    /** Where an overlay is ({ x, y }), or null before it was shown or moved. */
    overlayPosition: (id) => own("overlayPos", String(id)),
    /** The editor window's size: { width, height }. */
    screen: () => own("screen"),
    /** Runs fn every `ms` milliseconds (at least 50). Returns an id for xc.cancel. */
    every(ms, fn) {
      need(typeof fn === "function", "xc.every(ms, fn) needs a function");
      const id = nextTimer++;
      timers.set(id, { fn, repeat: true });
      own("timer", id, Number(ms) || 0, true);
      return id;
    },
    /** Runs fn once, after `ms` milliseconds. Returns an id for xc.cancel. */
    after(ms, fn) {
      need(typeof fn === "function", "xc.after(ms, fn) needs a function");
      const id = nextTimer++;
      timers.set(id, { fn, repeat: false });
      own("timer", id, Number(ms) || 0, false);
      return id;
    },
    /** Stops a timer from xc.every or xc.after. */
    cancel(id) {
      timers.delete(id);
      own("cancel", id);
    },
    /** Plays a sound file of the plugin (WAV, MP3 or Ogg), at `volume` from 0 to 1. */
    playSound: (file, o = {}) => own("sound", String(file), typeof o.volume === "number" ? o.volume : 1),
    /** Runs fn on "save" (the project was saved), "botEvent" (the running bot started, stopped, logged or ran a block) or "selection" (other blocks were selected). */
    on(event, fn) {
      need(event in listeners, `xc.on("${event}"): use "save", "botEvent" or "selection"`);
      need(typeof fn === "function", `xc.on("${event}", fn) needs a function`);
      listeners[event].push(fn);
    },

    /** The open project ({ name, root }), or null. */
    project: () => host("project"),
    /** Every workspace: [{ key, name }]. */
    workspaces: () => host("workspaces"),
    /** A workspace as saved ({ name, nodes, links, zones }), or null. */
    workspace: (key) => host("workspace", key),
    /** Replaces a workspace. It can be undone like any edit. */
    setWorkspace: (key, workspace) => host("setWorkspace", key, workspace),
    /** Every block in the project: [{ workspace, id, type, block, values, … }]. */
    blocks() {
      const out = [];
      for (const w of host("workspaces")) {
        const ws = host("workspace", w.key);
        for (const [id, node] of Object.entries((ws && ws.nodes) || {})) {
          out.push({ ...node, values: node.values || {}, workspace: w.key, id, type: String(node.block).split("@")[0] });
        }
      }
      return out;
    },
    /** The workspace open in the canvas, or null. */
    activeWorkspace: () => host("activeWorkspace"),
    /** The selected block ids in the active workspace. */
    selection: () => host("selection"),
    /** Opens a workspace, selects a block and centres the canvas on it. */
    reveal: (workspace, block) => host("reveal", workspace, block),
    /** Selects blocks of a workspace. */
    select: (workspace, blocks) => host("select", workspace, Array.isArray(blocks) ? blocks.map(String) : [String(blocks)]),
    /** Places a block ("core.log") on a workspace's canvas: { x, y, values }. Returns its id. It can be undone. */
    addBlock: (workspace, type, o = {}) => host("addBlock", workspace, String(type), Number(o.x) || 0, Number(o.y) || 0, o.values || {}),
    /** The display name of a block type such as "discord.send_message". */
    blockName: (type) => host("blockName", String(type).split("@")[0]),
    /** The project's `data` section, saved with the project. */
    data: { get: (key) => host("getData", key), set: (key, value) => host("setData", key, value === undefined ? null : value) },
    /** This plugin's own store, kept between sessions on this computer. */
    storage: { get: (key) => host("storageGet", key), set: (key, value) => host("storageSet", key, value === undefined ? null : value) },
    /** Writes to the Console. */
    log: log("info"),
    success: log("success"),
    warn: log("warn"),
    error: log("error"),
    /** Runs an editor command ("bot.run", "bot.stop", "file.save") or a plugin's command, after this handler. */
    run: (id) => host("run", id),
    botRunning: () => host("botRunning"),
    /** Installed block types matching words of `query` (all without): [{ type, name, category, kind, description }]. */
    blockTypes: (query) => host("blockTypes", text(query)),
    /** A block type ("core.log"): its fields, outputs and flows, or null. */
    blockType: (type) => host("blockType", String(type).split("@")[0]),
    /** A block of a workspace as it is now: its values and pins, with their types. */
    blockPins: (workspace, block) => host("blockPins", workspace, String(block)),
    /** Connects two pins ("3.next" to "4.in"), checked like a wire drawn on the canvas. It can be undone. */
    connect: (workspace, from, to) => host("connect", workspace, String(from), String(to)),
    /** Makes a workspace ({ folder }). Returns its key. */
    addWorkspace: (name, o = {}) => host("addWorkspace", text(name), o.folder ? text(o.folder) : ""),
    /** What the Problems panel lists: [{ source, message, workspace, block }]. */
    problems: () => host("problems"),
    /** The Console's last lines (50 without `lines`): [{ time, level, text, workspace, block }]. */
    console: (lines) => host("console", Number(lines) || 50),
    /** `text` in the app's language, from the plugins' translations (locales/*.toml). */
    t: (t) => host("t", text(t)),
    /** Puts text on the clipboard. */
    copyText: (t) => host("copyText", text(t)),
    /** XC Circuit's MCP server for AI agents: the clients it can be added to, adding it, its config entry and whether it's listening. */
    agents: Object.freeze({
      clients: () => host("agentClients"),
      install: (client) => host("agentInstall", text(client), false),
      remove: (client) => host("agentInstall", text(client), true),
      snippet: () => host("agentSnippet"),
      link: () => host("agentLink"),
    }),
    plugin: { name: __pluginName },
  };
  Object.freeze(xc.data);
  Object.freeze(xc.storage);
  globalThis.xc = Object.freeze(xc);
  globalThis.console = Object.freeze({ log: xc.log, info: xc.log, debug: xc.log, warn: xc.warn, error: xc.error });

  // What render(ui) fills in.
  function builder(out, fns) {
    const add = (item) => {
      out.push(item);
      return ui;
    };
    const handler = (fn) => {
      if (typeof fn !== "function") return null;
      fns.push(fn);
      return fns.length - 1;
    };
    const nested = (fn) => {
      const items = [];
      if (typeof fn === "function") fn(builder(items, fns));
      return items;
    };
    const ui = {
      heading: (t) => add({ t: "heading", text: text(t) }),
      text: (t, o = {}) => add({ t: "text", text: text(t), muted: !!o.muted, mono: !!o.mono, bold: !!o.bold, size: typeof o.size === "number" ? o.size : null, color: o.color ? String(o.color) : null }),
      muted: (t) => add({ t: "text", text: text(t), muted: true }),
      stat: (label, value) => add({ t: "stat", label: text(label), value: text(value) }),
      separator: () => add({ t: "separator" }),
      space: (size = 8) => add({ t: "space", size: Number(size) || 0 }),
      button: (label, onClick, o = {}) => add({ t: "button", text: text(label), on: handler(onClick), primary: !!o.primary, tooltip: o.tooltip ? text(o.tooltip) : null }),
      link: (label, onClick, o = {}) => add({ t: "link", text: text(label), on: handler(onClick), tooltip: o.tooltip ? text(o.tooltip) : null }),
      checkbox: (label, checked, onChange) => add({ t: "checkbox", text: text(label), value: !!checked, on: handler(onChange) }),
      input: (value, onChange, o = {}) => add({ t: "input", value: text(value), on: handler(onChange), hint: text(o.placeholder), multiline: !!o.multiline, key: o.key ? text(o.key) : null }),
      progress: (fraction, label) => add({ t: "progress", value: Number(fraction) || 0, text: text(label) }),
      image: (file, o = {}) => add({ t: "image", file: text(file), width: typeof o.width === "number" ? o.width : null, height: typeof o.height === "number" ? o.height : null, tint: o.tint ? String(o.tint) : null }),
      row: (fn) => add({ t: "row", items: nested(fn) }),
      toggle: (label, on, onChange) => add({ t: "toggle", text: text(label), value: !!on, on: handler(onChange) }),
      select: (value, choices, onChange, o = {}) =>
        add({
          t: "select",
          value: text(value),
          choices: Array.isArray(choices) ? choices.map((c) => (Array.isArray(c) ? [text(c[0]), text(c[1] ?? c[0])] : [text(c), text(c)])) : Object.entries(choices || {}).map(([v, l]) => [text(v), text(l)]),
          on: handler(onChange),
          width: typeof o.width === "number" ? o.width : 200,
        }),
      number: (value, onChange, o = {}) => add({ t: "number", value: Number(value) || 0, on: handler(onChange), min: typeof o.min === "number" ? o.min : null, max: typeof o.max === "number" ? o.max : null, step: typeof o.step === "number" ? o.step : null, suffix: text(o.suffix) }),
      slider: (value, min, max, onChange, o = {}) => add({ t: "slider", value: Number(value) || 0, min: Number(min) || 0, max: Number(max) || 0, on: handler(onChange), step: typeof o.step === "number" ? o.step : null, suffix: text(o.suffix) }),
      color: (value, onChange) => add({ t: "color", value: text(value), on: handler(onChange) }),
      note: (t, o = {}) => add({ t: "note", text: text(t), title: text(o.title), tone: text(o.tone) }),
      section: (title, fn, o = {}) => add({ t: "section", text: text(title), items: nested(fn), open: o.open !== false }),
    };
    return Object.freeze(ui);
  }

  // CommonJS modules from the plugin's own folder.
  const modules = new Map();
  const requireFrom = (dir) => (name) => {
    need(typeof name === "string", "require(name) needs a file name");
    const r = JSON.parse(__read(dir, name));
    if (r.error !== undefined) throw new Error(r.error);
    const { path, dir: inDir, source } = r.ok;
    if (modules.has(path)) return modules.get(path).exports;
    const module = { exports: {} };
    modules.set(path, module);
    __compile(path, source)(module, module.exports, requireFrom(inDir), path);
    return module.exports;
  };

  const reply = (fn) => (args) => {
    try {
      const value = fn(...JSON.parse(args));
      return JSON.stringify({ ok: value === undefined ? null : value });
    } catch (e) {
      return JSON.stringify({ error: fail(e) });
    }
  };
  const problem = (p) => {
    if (typeof p === "string") return { message: p, workspace: null, block: null };
    need(p && typeof p === "object", "a check returned something that isn't a problem");
    return { message: text(p.message), workspace: p.workspace ? text(p.workspace) : null, block: p.block ? text(p.block) : null };
  };

  globalThis.__xc = Object.freeze({
    load: reply((main) => {
      requireFrom("")(main);
      return {
        commands: [...commands.values()].map(({ id, title, description }) => ({ id, title, description })),
        panels: [...panels.values()].map(({ id, title }) => ({ id, title })),
        checks: checks.length,
        save: listeners.save.length,
        botEvent: listeners.botEvent.length,
        selection: listeners.selection.length,
        overlays: [...overlays.values()].map(({ id, x, y, width, height, frame, draggable, onClick }) => ({ id, x, y, width, height, frame, draggable, click: !!onClick })),
        tools: [...tools.values()].map(({ name, description, input }) => ({ name, description, input })),
      };
    }),
    tool: reply((name, args) => {
      const t = tools.get(name);
      need(t, `no tool "${name}"`);
      return t.run(args && typeof args === "object" ? args : {});
    }),
    command: reply((id) => {
      const c = commands.get(id);
      need(c, `no command "${id}"`);
      c.run();
    }),
    render: reply((id) => {
      const p = panels.get(id);
      need(p, `no panel "${id}"`);
      const items = [];
      const fns = [];
      p.render(builder(items, fns));
      handlers.set(id, fns);
      return items;
    }),
    event: reply((panel, index, value) => {
      const fn = (handlers.get(panel) || [])[index];
      if (fn) fn(value);
    }),
    check: reply(() => {
      const out = [];
      for (const fn of checks) {
        let found;
        try {
          found = fn();
        } catch (e) {
          out.push({ message: `A check failed: ${fail(e)}`, workspace: null, block: null });
          continue;
        }
        if (found === undefined || found === null) continue;
        for (const p of Array.isArray(found) ? found : [found]) out.push(problem(p));
      }
      return out;
    }),
    save: reply(() => {
      for (const fn of listeners.save) fn();
    }),
    botEvent: reply((event) => {
      for (const fn of listeners.botEvent) fn(event);
    }),
    selection: reply((event) => {
      for (const fn of listeners.selection) fn(event);
    }),
    timer: reply((id) => {
      const t = timers.get(id);
      if (!t) return;
      if (!t.repeat) timers.delete(id);
      t.fn();
    }),
    renderOverlay: reply((id) => {
      const o = overlays.get(id);
      need(o, `no overlay "${id}"`);
      const items = [];
      const fns = [];
      o.render(builder(items, fns));
      handlers.set(`overlay:${id}`, fns);
      return items;
    }),
    overlayClick: reply((id) => {
      const o = overlays.get(id);
      if (o && o.onClick) o.onClick();
    }),
  });
})();
