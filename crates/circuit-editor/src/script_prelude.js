// The `xc` object editor plugins use, and the calls XC Circuit makes into a
// plugin's script (`__xc`). Everything crosses to the editor as JSON text.
"use strict";
(() => {
  const host = (method, ...args) => {
    const r = JSON.parse(__host(method, JSON.stringify(args)));
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
  const checks = [];
  const listeners = { save: [], botEvent: [] };
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
    /** Runs fn on "save" (the project was saved) or "botEvent" (the running bot started, stopped, logged or ran a block). */
    on(event, fn) {
      need(event in listeners, `xc.on("${event}"): use "save" or "botEvent"`);
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
      row: (fn) => add({ t: "row", items: nested(fn) }),
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
      };
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
  });
})();
