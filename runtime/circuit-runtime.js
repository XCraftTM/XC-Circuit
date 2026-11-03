// XC Circuit runtime. Runs a project's block graph in Node.js 22+.
//
// Used two ways:
//   node circuit-runtime.js --bundle bundle.json --deps <node_modules dir> [--debug]   (Run from the editor)
//   an exported index.js that defines __XC_BUNDLE__ and then contains this file         (exported bot)
//
// With --debug, protocol events are written to stdout as lines starting with "@@xc ".
"use strict";

const fs = require("fs");
const path = require("path");
const Module = require("module");

const ACTION_IN = "in";
const PROTOCOL = "@@xc ";

function start(bundle, opts) {
  const rt = new Runtime(bundle, opts);
  rt.start().catch((e) => {
    rt.log.error(`Bot stopped: ${e && e.message ? e.message : e}`);
    process.exitCode = 1;
  });
  return rt;
}

class Runtime {
  constructor(bundle, opts) {
    this.bundle = bundle;
    this.opts = opts;
    this.debug = Boolean(opts.debug);
    this.startedAt = Date.now();
    this.require = Module.createRequire(path.join(opts.depsRoot, "package.json"));
    this.blocks = new Map(); // block id -> definition object
    this.libs = new Map(); // lib id -> api
    this.workspaces = new Map(); // key -> graph
    this.globalVars = new Map();
    this.workspaceVars = new Map();
    this.errorHandlers = [];
    this.signalHandlers = new Map();
    /** Call Workspace: name -> handlers that start a run and resolve to its return value. */
    this.callHandlers = new Map();
    this.startHandlers = [];
    // Opened in start(), after the saved data was checked and backed up.
    this.store = null;
    this.runCounter = 0;
    this.log = makeLogger(this, null);
  }

  emit(type, data) {
    if (this.debug) process.stdout.write(PROTOCOL + JSON.stringify({ t: type, ...data }) + "\n");
  }

  secret(name) {
    const env = process.env[`XC_SECRET_${String(name).toUpperCase()}`];
    if (env) return env;
    if (name === "token") return process.env.DISCORD_TOKEN || process.env.DBB_TOKEN;
    return undefined;
  }

  setting(name) {
    return (this.bundle.project.settings || {})[name];
  }

  /** Union of `intents` of every event block placed in the project. */
  intents() {
    const out = new Set();
    for (const ws of this.workspaces.values()) {
      for (const node of ws.nodes.values()) {
        for (const i of node.def.intents || []) out.add(i);
      }
    }
    return [...out];
  }

  /** Every placed node with the given block id, across all workspaces. */
  nodesOf(blockId) {
    const out = [];
    for (const ws of this.workspaces.values()) {
      for (const node of ws.nodes.values()) if (node.blockId === blockId) out.push(node);
    }
    return out;
  }

  loadModule(source, file) {
    const module = { exports: {} };
    // `@xc-circuit/sdk` only helps editors with types; at run time defineBlock returns the block.
    const req = (name) => (name === "@xc-circuit/sdk" ? { defineBlock: (b) => b } : this.require(name));
    new Function("module", "exports", "require", "__filename", "__dirname", source)(
      module,
      module.exports,
      req,
      file,
      path.dirname(file),
    );
    const out = module.exports;
    return out && out.default ? out.default : out;
  }

  async start() {
    // First: check the saved data, back it up, and ask if it is empty.
    await prepareStoreFile(this.opts.dataDir, this);
    this.store = new Store(this.opts.dataDir, this);
    if (this.store.note) this.log.warn(this.store.note);
    this.backupTimer = setInterval(() => backupStore(this.opts.dataDir, this, true), BACKUP_EVERY_MS);
    this.backupTimer.unref?.();
    this.log.info(`Runtime ready · Node ${process.version}`);
    for (const [id, b] of Object.entries(this.bundle.blocks)) {
      try {
        this.blocks.set(id, ungroupBlock(this.loadModule(esmToCommonJs(b.source), b.file)));
      } catch (e) {
        this.log.error(`Could not load block ${id}: ${e.message}`);
      }
    }
    for (const [key, ws] of Object.entries(this.bundle.workspaces)) {
      this.workspaces.set(key, new Graph(this, key, ws));
    }

    // Runtime libraries (such as the Discord client) set up before events subscribe.
    const libs = [];
    for (const lib of this.bundle.libs || []) {
      try {
        const mod = this.loadModule(lib.source, lib.file);
        const api = mod.setup ? (await mod.setup(this)) || {} : {};
        this.libs.set(mod.id, api);
        libs.push({ mod, api });
      } catch (e) {
        this.log.error(`Runtime library ${lib.file} failed: ${e.message}`);
      }
    }

    for (const ws of this.workspaces.values()) {
      for (const node of ws.nodes.values()) {
        if (node.def.setup) await node.def.setup(this.ctxFor(null, node));
      }
    }
    for (const ws of this.workspaces.values()) ws.listen();

    for (const { mod, api } of libs) {
      if (!mod.start) continue;
      try {
        await mod.start(this, api);
      } catch (e) {
        this.log.error(e.message);
      }
    }
    for (const h of this.startHandlers) h({});
    this.emit("ready", {});
  }

  /**
   * What a block gets as `ctx`. `path` is where it runs: the Try Zones it is
   * in and their abort signal. Each path carries its own, so blocks running
   * side by side never see each other's zones.
   */
  ctxFor(run, node, path = NO_ZONES) {
    const rt = this;
    const graph = node.graph;
    const ctx = {
      fields: node.inline(),
      event: run ? run.event : undefined,
      log: makeLogger(rt, node),
      store: rt.store.api(),
      vars: {
        run: mapApi(run ? run.vars : new Map()),
        workspace: mapApi(getOr(rt.workspaceVars, graph.key, () => new Map())),
        global: mapApi(rt.globalVars),
      },
      signal: {
        emit: (name, data) => rt.fireSignal(name, data),
      },
      abortSignal: path.signal,
      next: async (flow, outputs) => {
        if (!run) throw new Error("ctx.next() can only be used while a block runs");
        if (outputs) run.setOutputs(node.id, outputs);
        await graph.follow(run, node.id, flow, path);
      },
      set: (output, value) => {
        if (run) run.setOutputs(node.id, { [output]: value });
      },
      /** Reads the block's inputs again (for loops whose condition changes). */
      resolve: async () => (run ? node.resolve(run, new Map(), path) : node.inline()),
      /** Sets what this run gives back to a Call Workspace block. */
      returnValue: (value) => {
        if (run) run.returned = value;
      },
      bot: {
        get uptime() {
          return Date.now() - rt.startedAt;
        },
        stop: () => process.exit(0),
        restart: () => process.exit(75),
      },
      runtime: {
        onError: (h) => rt.errorHandlers.push({ h, node }),
        onSignal: (name, h) => getOr(rt.signalHandlers, name, () => []).push(h),
        /** `h(data)` starts a run and resolves to what it returns. */
        onCall: (name, h) => getOr(rt.callHandlers, name, () => []).push(h),
        /** Runs the workspace(s) listening for `name` and resolves to the first one's return value. */
        call: async (name, data) => {
          const handlers = rt.callHandlers.get(name) || [];
          if (!handlers.length) throw new Error(`Nothing is listening for the call "${name}" (add a When Called block)`);
          const results = await Promise.all(handlers.map((h) => h(data || {})));
          return results[0];
        },
        onStart: (h) => rt.startHandlers.push(h),
        setting: (name) => rt.setting(name),
      },
      util: UTIL,
      require: (name) => rt.require(name),
      /** Project info: name, settings and project-editor data such as slashCommands. */
      project: rt.bundle.project,
      /** Fills {placeholders} in text from the event, run variables and `vars`. */
      fill: (text, vars) => fillTemplate(String(text ?? ""), run, vars || {}),
    };
    for (const [id, api] of rt.libs) ctx[id] = api;
    return ctx;
  }

  fireSignal(name, data) {
    for (const h of this.signalHandlers.get(name) || []) h(data || {});
  }

  /** An error nobody caught: On Error blocks, otherwise the log. */
  /**
   * An error no Try Zone caught, from a run started by `event`: goes to the
   * On Error blocks, or the log. An error in a run that On Error started is
   * only logged, so a failing On Error path can't set itself off again.
   */
  uncaught(err, node, event) {
    const details = describeError(err, node);
    this.emit("node", { ws: node.graph.key, node: node.id, phase: "error", error: details.message });
    const where = `${node.graph.name} › ${node.def.name || node.blockId} (#${node.id})`;
    const fromOnError = this.errorHandlers.some((e) => e.node === event);
    if (!this.errorHandlers.length || fromOnError) {
      this.log.error(`${where}: ${details.message}${fromOnError ? " (in an On Error path)" : ""}`);
      return;
    }
    for (const { h } of this.errorHandlers) {
      try {
        Promise.resolve(h({ message: details.message, details, block: details.block })).catch((e) => this.log.error(`On Error failed: ${e.message}`));
      } catch (e) {
        this.log.error(`On Error failed: ${e.message}`);
      }
    }
  }
}

/**
 * Lets block files use `export default { … }`: only a line starting with it
 * counts, never the words inside a string or a comment.
 */
function esmToCommonJs(source) {
  return source.replace(/^([ \t]*)export\s+default(?=[\s{(])/m, "$1module.exports =");
}

/** How a block file groups what describes it (the editor reads the same). */
const BLOCK_GROUPS = {
  info: ["name", "description", "author", "version", "docs", "deprecated"],
  search: ["category", "aliases", "tags"],
  look: ["icon", "width", "preview"],
  requires: ["intents", "dependencies"],
};

/** Moves the keys in a block's groups (`info`, `search`, …) to the top. */
function ungroupBlock(block) {
  if (!block || typeof block !== "object") return block;
  for (const [group, keys] of Object.entries(BLOCK_GROUPS)) {
    const found = block[group];
    if (!found || typeof found !== "object") continue;
    for (const key of keys) if (key in found && !(key in block)) block[key] = found[key];
  }
  return block;
}

/** Where a block runs outside every Try Zone. */
const NO_ZONES = Object.freeze({ zones: [], signal: undefined });

class Graph {
  constructor(rt, key, data) {
    this.rt = rt;
    this.key = key;
    this.name = data.name || key;
    this.nodes = new Map();
    this.zones = new Map(Object.entries(data.zones || {}));
    this.flowsOut = new Map(); // "node.flow" -> [target node ids]
    this.inputs = new Map(); // "node.pin" -> [{ node, pin }]
    for (const [id, n] of Object.entries(data.nodes || {})) {
      const blockId = String(n.block).split("@")[0];
      const def = rt.blocks.get(blockId);
      if (!def) {
        rt.log.warn(`${this.name}: block ${blockId} (#${id}) is not available and is skipped`);
        continue;
      }
      this.nodes.set(id, new GraphNode(this, id, blockId, def, n));
    }
    for (const link of data.links || []) {
      const [from, to] = link.split(">").map((s) => s.trim());
      const [fromNode, fromPin] = splitPin(from);
      const [toNode, toPin] = splitPin(to);
      if (toPin === ACTION_IN) getOr(this.flowsOut, `${fromNode}.${fromPin}`, () => []).push(toNode);
      else getOr(this.inputs, `${toNode}.${toPin}`, () => []).push({ node: fromNode, pin: fromPin });
    }
  }

  listen() {
    for (const node of this.nodes.values()) {
      if (node.def.kind !== "event" || !node.def.listen) continue;
      const ctx = this.rt.ctxFor(null, node);
      // Resolves to the run's return value once it has finished.
      const emit = (outputs) => this.trigger(node, outputs || {});
      try {
        node.def.listen(ctx, emit);
      } catch (e) {
        this.rt.log.error(`${this.name} › ${node.def.name}: listen() failed: ${e.message}`);
      }
    }
  }

  trigger(node, outputs) {
    const run = new Run(this, outputs);
    run.setOutputs(node.id, outputs);
    this.rt.emit("node", { ws: this.key, node: node.id, phase: "enter", run: run.id });
    this.rt.emit("node", { ws: this.key, node: node.id, phase: "exit", run: run.id, ms: 0 });
    return this.follow(run, node.id, "next").then(
      () => run.returned,
      (e) => {
        this.rt.uncaught(e, e.__xcNode || node, node);
        return undefined;
      },
    );
  }

  async follow(run, nodeId, flow, path = NO_ZONES) {
    const targets = this.flowsOut.get(`${nodeId}.${flow}`) || [];
    await Promise.all(targets.map((t) => this.execute(run, t, path)));
  }

  /** Runs an action block, entering its Try Zone when needed. */
  async execute(run, nodeId, path) {
    const node = this.nodes.get(nodeId);
    if (!node) return;
    const zone = node.data.zone;
    if (zone && this.zones.has(zone) && !path.zones.includes(zone)) {
      return this.runZone(run, zone, nodeId, path);
    }
    if (!zone && path.zones.length) {
      // An Action wire may only leave a zone through its own outputs; treat it as leaving.
      return this.execute(run, nodeId, NO_ZONES);
    }
    return this.executeNode(run, node, path);
  }

  async executeNode(run, node, path) {
    const t0 = performance.now();
    this.rt.emit("node", { ws: this.key, node: node.id, phase: "enter", run: run.id });
    try {
      // A zone that timed out stops at its next block.
      if (path.signal && path.signal.aborted) throw new Error("timed out");
      const fields = await node.resolve(run, new Map(), path);
      const ctx = this.rt.ctxFor(run, node, path);
      const result = await node.def.run({ fields, ctx, node: { id: node.id, workspace: this.key } });
      if (result && typeof result === "object") run.setOutputs(node.id, result);
      this.rt.emit("node", { ws: this.key, node: node.id, phase: "exit", run: run.id, ms: +(performance.now() - t0).toFixed(1) });
      if (!node.def.flows) await this.follow(run, node.id, "next", path);
    } catch (e) {
      if (!e.__xcReported) {
        e.__xcReported = true;
        e.__xcNode = e.__xcNode || node;
        this.rt.emit("node", { ws: this.key, node: node.id, phase: "error", run: run.id, error: String(e.message || e) });
      }
      throw e;
    }
  }

  async runZone(run, zoneId, entryId, path) {
    const zone = this.zones.get(zoneId);
    const attempts = 1 + Math.max(0, zone.retry || 0);
    let lastError;
    for (let attempt = 1; attempt <= attempts; attempt++) {
      const controller = new AbortController();
      // An outer zone timing out stops this one too.
      const signal = path.signal ? AbortSignal.any([path.signal, controller.signal]) : controller.signal;
      let timer;
      try {
        const work = this.execute(run, entryId, { zones: [...path.zones, zoneId], signal });
        if (zone.timeoutMs) {
          const timeout = new Promise((_, reject) => {
            timer = setTimeout(() => {
              controller.abort();
              reject(Object.assign(new Error(`timed out after ${zone.timeoutMs} ms`), { code: "TIMEOUT" }));
            }, zone.timeoutMs);
          });
          await Promise.race([work, timeout]);
        } else {
          await work;
        }
        clearTimeout(timer);
        this.rt.emit("zone", { ws: this.key, zone: zoneId, phase: "done", run: run.id });
        return this.follow(run, zoneId, "done", path);
      } catch (e) {
        clearTimeout(timer);
        controller.abort();
        lastError = e;
        if (attempt < attempts) {
          this.rt.log.warn(`${this.name} › Try ${zone.title || zoneId}: ${e.message} · retry ${attempt}/${attempts - 1}`);
        }
      }
    }
    const details = describeError(lastError, lastError.__xcNode);
    run.setOutputs(zoneId, { message: details.message, details });
    this.rt.emit("zone", { ws: this.key, zone: zoneId, phase: "error", run: run.id, error: details.message });
    const handlers = this.flowsOut.get(`${zoneId}.error`) || [];
    if (!handlers.length) {
      this.rt.log.warn(`${this.name} › Try ${zone.title || zoneId} caught: ${details.message}`);
    }
    return this.follow(run, zoneId, "error", path);
  }

  /** Reads an output: from blocks that already ran in this run, or by computing a value block. */
  /**
   * An output of a block. A value block runs when it is read, once for all
   * the inputs being filled together (`memo`): one Random Number wired into
   * two inputs gives both the same number. The next block reads it afresh,
   * so in a loop it can change on every pass.
   */
  async read(run, nodeId, pin, memo = new Map(), path = NO_ZONES) {
    const done = run.outputs.get(nodeId);
    if (done && pin in done) return done[pin];
    const node = this.nodes.get(nodeId);
    if (!node) return undefined;
    if (node.def.kind !== "value") return done ? done[pin] : undefined;
    if (!memo.has(nodeId)) {
      memo.set(
        nodeId,
        (async () => {
          const fields = await node.resolve(run, memo, path);
          return (await node.def.run({ fields, ctx: this.rt.ctxFor(run, node, path), node: { id: node.id, workspace: this.key } })) || {};
        })(),
      );
    }
    return (await memo.get(nodeId))[pin];
  }
}

class GraphNode {
  constructor(graph, id, blockId, def, data) {
    this.graph = graph;
    this.id = id;
    this.blockId = blockId;
    this.def = def;
    this.data = data;
  }

  /** Inline values with defaults, as typed in the editor. */
  inline() {
    const out = {};
    for (const [k, f] of Object.entries(this.def.fields || {})) {
      if (f && "default" in f) out[k] = f.default;
    }
    return Object.assign(out, this.data.values || {});
  }

  /** Fields for run(): wires first, then inline values, with {placeholders} filled. */
  async resolve(run, memo = new Map(), path = NO_ZONES) {
    const fields = this.inline();
    const wired = new Set();
    for (const [key, sources] of this.graph.inputs) {
      const [nodeId, pin] = splitPin(key);
      if (nodeId !== this.id) continue;
      const values = await Promise.all(sources.map((s) => this.graph.read(run, s.node, s.pin, memo, path)));
      const def = (this.def.fields || {})[pin];
      fields[pin] = def && def.multi ? values.flat() : values[0];
      wired.add(pin);
    }
    // Values wired into a text field's {name} inputs (pins that aren't fields).
    const placed = {};
    for (const pin of wired) if (!(pin in (this.def.fields || {}))) placed[pin] = fields[pin];
    for (const [k, f] of Object.entries(this.def.fields || {})) {
      if (!f) continue;
      if (templated(f) && !wired.has(k) && typeof fields[k] === "string") fields[k] = fillTemplate(fields[k], run, fields, placed);
      fields[k] = coerce(fields[k], f.type);
    }
    return fields;
  }
}

class Run {
  constructor(graph, event) {
    this.id = ++graph.rt.runCounter;
    this.graph = graph;
    this.event = event;
    this.outputs = new Map();
    this.vars = new Map();
  }

  setOutputs(nodeId, outputs) {
    this.outputs.set(nodeId, Object.assign(this.outputs.get(nodeId) || {}, outputs));
  }
}

/** Whether ${…} in a field is filled in: plain text fields, unless `template: false`. */
function templated(f) {
  if (typeof f.template === "boolean") return f.template;
  return ["text", "textarea", "string"].includes(String(f.type || "").replace(/\?$/, "")) && !f.choices;
}

/**
 * Fills each ${…} in a text with the value of the JavaScript expression in it.
 * Names in the expression are looked up in `placed` (values wired into the
 * text's inputs), the event, run variables, the block's fields and the latest
 * output of that name earlier in the run; other names are JavaScript's own
 * (Math, Date, JSON …). #4 is block #4's outputs in this run: ${#4.message.url}.
 * Empty values (null, undefined) give "". A mistake in an expression throws,
 * naming the expression.
 */
function fillTemplate(text, run, fields, placed = {}) {
  if (!text.includes("${")) return text;
  let out = "";
  let from = 0;
  for (;;) {
    const open = text.indexOf("${", from);
    const close = open < 0 ? -1 : expressionEnd(text, open + 2);
    if (close < 0) return out + text.slice(from);
    const expr = text.slice(open + 2, close);
    out += text.slice(from, open);
    let value;
    try {
      value = evaluate(expr, run, fields, placed);
    } catch (e) {
      throw new Error(`\${${expr}}: ${e.message}`);
    }
    out += value == null ? "" : String(value);
    from = close + 1;
  }
}

/** Where the `}` closing an expression that starts at `from` is, or -1. */
function expressionEnd(text, from) {
  let depth = 0;
  for (let i = from; i < text.length; i++) {
    const c = text[i];
    if (c === '"' || c === "'") i = stringEnd(text, i, c);
    else if (c === "`") i = templateEnd(text, i);
    else if (c === "{") depth++;
    else if (c === "}" && depth-- === 0) return i;
  }
  return -1;
}

function stringEnd(text, i, quote) {
  for (let j = i + 1; j < text.length; j++) {
    if (text[j] === "\\") j++;
    else if (text[j] === quote) return j;
  }
  return text.length;
}

function templateEnd(text, i) {
  for (let j = i + 1; j < text.length; j++) {
    if (text[j] === "\\") j++;
    else if (text[j] === "`") return j;
    else if (text[j] === "$" && text[j + 1] === "{") {
      const end = expressionEnd(text, j + 2);
      if (end < 0) return text.length;
      j = end;
    }
  }
  return text.length;
}

/** An expression with each #4 (outside strings) turned into a lookup of block #4's outputs. */
function blockReferences(expr) {
  let out = "";
  for (let i = 0; i < expr.length; i++) {
    const c = expr[i];
    if (c === '"' || c === "'" || c === "`") {
      const end = c === "`" ? templateEnd(expr, i) : stringEnd(expr, i, c);
      out += expr.slice(i, end + 1);
      i = end;
    } else if (c === "#" && /[\w]/.test(expr[i + 1] || "")) {
      const id = expr.slice(i + 1).match(/^\w+/)[0];
      out += `__blocks[${JSON.stringify(id)}]`;
      i += id.length;
    } else out += c;
  }
  return out;
}

// Each expression is compiled once.
const compiled = new Map();

function evaluate(expr, run, fields, placed) {
  const code = blockReferences(expr);
  let fn = compiled.get(code);
  if (!fn) {
    // Not strict mode, for `with`: names resolve through the scope below first.
    fn = new Function("__scope", "__blocks", `with (__scope) { return (${code}\n); }`);
    compiled.set(code, fn);
  }
  const lookup = (name) => {
    if (name in placed) return placed[name];
    if (run && run.event && name in run.event) return run.event[name];
    if (run && run.vars.has(name)) return run.vars.get(name);
    if (name in fields) return fields[name];
    return run ? lastOutput(run, name) : NOT_FOUND;
  };
  const scope = new Proxy(Object.create(null), {
    has: (_, name) => typeof name === "string" && !name.startsWith("__") && lookup(name) !== NOT_FOUND,
    get: (_, name) => (typeof name === "string" ? lookup(name) : undefined),
  });
  const blocks = new Proxy(Object.create(null), { get: (_, id) => (run ? run.outputs.get(String(id)) : undefined) });
  return fn(scope, blocks);
}

const NOT_FOUND = Symbol("not found");

/** The most recently set output with this name from any block earlier in the run. */
function lastOutput(run, name) {
  let found = NOT_FOUND;
  for (const outputs of run.outputs.values()) if (outputs && name in outputs) found = outputs[name];
  return found;
}

function coerce(value, type) {
  if (value == null) return value;
  const t = String(type || "any").replace(/\?$/, "");
  // One value where a list goes is a list of one (an embed into Embeds).
  // Collections, Maps and Sets stay as they are: blocks walk those themselves.
  if (t.startsWith("list<") && !Array.isArray(value) && (typeof value !== "object" || typeof value[Symbol.iterator] !== "function")) return [value];
  if ((t === "text" || t === "textarea") && typeof value !== "string" && typeof value !== "object") return String(value);
  if ((t === "text" || t === "textarea") && typeof value === "object" && !Array.isArray(value) && value.toString !== Object.prototype.toString) {
    return String(value);
  }
  if (t === "number" && typeof value === "string" && value.trim() !== "" && !isNaN(Number(value))) return Number(value);
  if (t === "boolean" && typeof value === "string") return !["", "false", "0", "no", "off"].includes(value.trim().toLowerCase());
  return value;
}

function describeError(err, node) {
  const e = err || {};
  return {
    message: String(e.message || e),
    code: e.code ?? e.status ?? null,
    block: node ? node.def.name || node.blockId : null,
    node: node ? node.id : null,
    workspace: node ? node.graph.name : null,
    stack: e.stack || null,
  };
}

/** Every store, so pending changes can be written when the process ends. */
const openStores = new Set();

/** How many backups of the saved data are kept, and how often more are made. */
const BACKUPS_KEPT = 10;
const BACKUP_EVERY_MS = 6 * 60 * 60 * 1000;

function storePaths(dataDir) {
  const backups = path.join(dataDir, "backups");
  return { file: path.join(dataDir, "store.json"), backups };
}

/** Backups of the saved data, newest first, each with its time. */
function listStoreBackups(dataDir) {
  const { backups } = storePaths(dataDir);
  let names = [];
  try {
    names = fs.readdirSync(backups).filter((n) => /^store-.*\.json$/.test(n));
  } catch {
    return [];
  }
  return names
    .map((name) => {
      const file = path.join(backups, name);
      let time = 0;
      try {
        time = fs.statSync(file).mtimeMs;
      } catch {}
      return { name, file, time };
    })
    .sort((a, b) => b.name.localeCompare(a.name));
}

/** The saved data's text, or null when there is no file. */
function readStoreText(file) {
  try {
    return fs.readFileSync(file, "utf8");
  } catch {
    return null;
  }
}

function isValidStore(text) {
  try {
    const v = JSON.parse(text);
    return Boolean(v) && typeof v === "object" && !Array.isArray(v);
  } catch {
    return false;
  }
}

/**
 * Copies store.json into data/backups when it holds valid JSON, keeping the
 * newest BACKUPS_KEPT copies. A damaged or empty file is never backed up, so
 * it can't push good backups out.
 */
function backupStore(dataDir, rt, flush = false) {
  const { file, backups } = storePaths(dataDir);
  // A running bot writes its pending changes first, so the backup is current.
  if (flush && rt && rt.store) rt.store.flushPending();
  const text = readStoreText(file);
  if (text === null || !text.trim() || !isValidStore(text)) return null;
  try {
    fs.mkdirSync(backups, { recursive: true });
    const stamp = new Date().toISOString().replace(/[:.]/g, "-");
    let target = path.join(backups, `store-${stamp}.json`);
    for (let i = 2; fs.existsSync(target); i++) target = path.join(backups, `store-${stamp}-${i}.json`);
    fs.writeFileSync(target, text);
    for (const old of listStoreBackups(dataDir).slice(BACKUPS_KEPT)) fs.rmSync(old.file, { force: true });
    return target;
  } catch (e) {
    if (rt && rt.log) rt.log.warn(`Could not back up saved data: ${e.message}`);
    return null;
  }
}

/**
 * Runs before the bot starts: backs up valid saved data, and when the file is
 * empty (or damaged) asks whether to restore a backup or start from the
 * default template.
 */
async function prepareStoreFile(dataDir, rt) {
  const { file } = storePaths(dataDir);
  const text = readStoreText(file);
  if (text === null) return;
  if (text.trim() && isValidStore(text)) {
    backupStore(dataDir, rt);
    return;
  }
  const damaged = Boolean(text.trim());
  if (damaged) {
    // Kept as it is, next to the new file.
    let aside = file.replace(/\.json$/, `.damaged-${Date.now()}.json`);
    for (let i = 2; fs.existsSync(aside); i++) aside = file.replace(/\.json$/, `.damaged-${Date.now()}-${i}.json`);
    try {
      fs.renameSync(file, aside);
      rt.log.warn(`Saved data couldn't be read; the file was kept as ${path.basename(aside)}.`);
    } catch (e) {
      rt.log.error(`Saved data couldn't be read or moved aside (${e.message}); it is left untouched.`);
      return;
    }
  }
  const valid = listStoreBackups(dataDir).filter((b) => isValidStore(readStoreText(b.file) || ""));
  const choices = valid.map((b, i) => ({ value: `backup:${b.name}`, label: `Restore the backup from ${new Date(b.time).toLocaleString()}${i === 0 ? " (newest)" : ""}` }));
  choices.push({ value: "template", label: "Start with empty saved data (the default template)" });
  const question = damaged ? "The bot's saved data (store.json) was damaged. What should it start with?" : "The bot's saved data (store.json) is empty. What should it start with?";
  const answer = await askUser(rt, "store-empty", question, choices);
  if (answer.startsWith("backup:")) {
    const chosen = valid.find((b) => `backup:${b.name}` === answer);
    fs.copyFileSync(chosen.file, file);
    rt.log.success(`Saved data restored from ${chosen.name}.`);
  } else {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, "{}\n");
    rt.log.info("Saved data starts empty.");
  }
}

/** Questions waiting for XC Circuit's answer (bots started from the editor). */
const pendingAnswers = new Map();

/**
 * Asks the person running the bot to pick one of `choices` ({value, label}).
 * From XC Circuit: a dialog in the app. In a terminal: a numbered list. With
 * nobody to ask (a service): the first choice, logged.
 */
function askUser(rt, id, question, choices) {
  if (rt.debug && process.argv.includes("--stop-on-input")) {
    rt.emit("ask", { id, text: question, choices });
    return new Promise((resolve) => pendingAnswers.set(id, (value) => resolve(choices.some((c) => c.value === value) ? value : choices[0].value)));
  }
  if (process.stdin.isTTY) {
    const readline = require("readline");
    const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
    const list = choices.map((c, i) => `  ${i + 1}) ${c.label}`).join("\n");
    return new Promise((resolve) => {
      let answered = false;
      // Input closed (Ctrl+D, or a pipe ending) before an answer: take the first.
      rl.on("close", () => {
        if (!answered) {
          rt.log.warn(`${question} No answer, so: ${choices[0].label}.`);
          resolve(choices[0].value);
        }
      });
      const ask = () =>
        rl.question(`\n${question}\n${list}\nChoose 1-${choices.length}: `, (reply) => {
          const n = Number.parseInt(String(reply).trim(), 10);
          if (n >= 1 && n <= choices.length) {
            answered = true;
            rl.close();
            resolve(choices[n - 1].value);
          } else {
            ask();
          }
        });
      ask();
    });
  }
  rt.log.warn(`${question} Nobody can answer here, so: ${choices[0].label}.`);
  return Promise.resolve(choices[0].value);
}

/**
 * The key of a stored-data bucket: `user:123`, `member:<server>:<user>`.
 * With two ids (members), a `:` inside one is written as %3A, so
 * ("1:2", "3") and ("1", "2:3") don't share a bucket. A single id is kept
 * as it is, as it always was.
 */
function storeKey([scope, ...ids]) {
  return [scope, ...ids.map((id) => (ids.length > 1 ? keyPart(id) : String(id?.id ?? id)))].join(":");
}

function keyPart(id) {
  return String(id?.id ?? id).replace(/[%:]/g, encodeURIComponent);
}

/** Saved data (ctx.store): one JSON file, written safely and in batches. */
class Store {
  constructor(dir, rt) {
    this.rt = rt;
    this.file = path.join(dir, "store.json");
    this.data = {};
    /** A message for the log once it exists, e.g. about a damaged file. */
    this.note = null;
    let text = null;
    try {
      text = fs.readFileSync(this.file, "utf8");
    } catch {
      // No saved data yet.
    }
    if (text !== null) {
      try {
        const parsed = JSON.parse(text);
        if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("not an object");
        this.data = parsed;
      } catch (e) {
        // Never overwrite data that can't be read: keep it next to the new file.
        let backup = this.file.replace(/\.json$/, `.damaged-${Date.now()}.json`);
        for (let i = 2; fs.existsSync(backup); i++) backup = this.file.replace(/\.json$/, `.damaged-${Date.now()}-${i}.json`);
        try {
          fs.renameSync(this.file, backup);
          this.note = `Saved data couldn't be read (${e.message}), so it starts empty. The old file was kept as ${path.basename(backup)}.`;
        } catch (err) {
          this.note = `Saved data couldn't be read (${e.message}) or moved aside (${err.message}); it won't be saved over.`;
          this.readOnly = true;
        }
      }
    }
    this.timer = null;
    openStores.add(this);
  }

  save() {
    if (this.timer || this.readOnly) return;
    this.timer = setTimeout(() => this.flush(), 250);
  }

  /** Writes pending changes now. */
  flush() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    if (this.readOnly) return;
    try {
      fs.mkdirSync(path.dirname(this.file), { recursive: true });
      const tmp = this.file + ".tmp";
      fs.writeFileSync(tmp, JSON.stringify(this.data, null, 2));
      fs.renameSync(tmp, this.file);
    } catch (e) {
      if (this.rt.log) this.rt.log.error(`Could not save stored data: ${e.message}`);
      else console.error(`Could not save stored data: ${e.message}`);
    }
  }

  /** Writes only if a save was waiting. */
  flushPending() {
    if (this.timer) this.flush();
  }

  scope(...parts) {
    const key = storeKey(parts);
    const bucket = () => getOrObj(this.data, key);
    return {
      get: async (k, fallback) => (k in bucket() ? bucket()[k] : fallback),
      has: async (k) => k in bucket(),
      set: async (k, v) => {
        bucket()[k] = v;
        this.save();
      },
      delete: async (k) => {
        delete bucket()[k];
        this.save();
      },
      inc: async (k, by = 1) => {
        const b = bucket();
        b[k] = (Number(b[k]) || 0) + Number(by);
        this.save();
        return b[k];
      },
      push: async (k, v) => {
        const b = bucket();
        b[k] = Array.isArray(b[k]) ? [...b[k], v] : [v];
        this.save();
        return b[k];
      },
      keys: async () => Object.keys(bucket()),
    };
  }

  /** Every saved bucket of a scope, as { id, values }: all users, or one server's members. */
  each(scope, serverId) {
    const member = scope === "member";
    const prefix = member ? `member:${keyPart(serverId)}:` : `${scope}:`;
    return Object.entries(this.data)
      .filter(([k]) => k.startsWith(prefix))
      .map(([k, values]) => {
        const id = k.slice(prefix.length);
        return { id: member ? decodeURIComponent(id) : id, values };
      });
  }

  api() {
    return {
      global: () => this.scope("global"),
      server: (id) => this.scope("server", id),
      user: (id) => this.scope("user", id),
      member: (serverId, userId) => this.scope("member", serverId, userId),
      channel: (id) => this.scope("channel", id),
      ns: (name) => this.scope("ns", name),
      each: (scope, serverId) => this.each(scope, serverId),
    };
  }
}

const UTIL = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  isURL: (s) => {
    try {
      return Boolean(new URL(String(s)));
    } catch {
      return false;
    }
  },
  id: (n = 10) => Array.from({ length: n }, () => "abcdefghijklmnopqrstuvwxyz0123456789"[Math.floor(Math.random() * 36)]).join(""),
  shuffle: (list) => {
    // Fisher-Yates: every order is equally likely (sorting by a random
    // comparison is not).
    const out = [...list];
    for (let i = out.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [out[i], out[j]] = [out[j], out[i]];
    }
    return out;
  },
  typeOf: (v) => (v === null ? "null" : Array.isArray(v) ? "list" : typeof v),
};

function makeLogger(rt, node) {
  const write = (level) => (...parts) => {
    const text = parts.map((p) => (typeof p === "string" ? p : inspect(p))).join(" ");
    if (rt.debug) {
      rt.emit("log", { level, text, ws: node ? node.graph.key : undefined, node: node ? node.id : undefined });
    } else {
      const colors = { info: 36, success: 32, warn: 33, error: 31, debug: 90 };
      const time = new Date().toLocaleTimeString();
      (level === "error" ? console.error : console.log)(`\x1b[90m${time}\x1b[0m \x1b[${colors[level] || 0}m${level.toUpperCase().padEnd(7)}\x1b[0m ${text}`);
    }
  };
  return { info: write("info"), success: write("success"), warn: write("warn"), error: write("error"), debug: write("debug") };
}

function inspect(v) {
  try {
    return require("util").inspect(v, { depth: 2, breakLength: 120 });
  } catch {
    return String(v);
  }
}

function mapApi(map) {
  return {
    get: (k, fallback) => (map.has(k) ? map.get(k) : fallback),
    set: (k, v) => void map.set(k, v),
    delete: (k) => map.delete(k),
    has: (k) => map.has(k),
  };
}

function getOr(map, key, make) {
  if (!map.has(key)) map.set(key, make());
  return map.get(key);
}

function getOrObj(obj, key) {
  if (!obj[key] || typeof obj[key] !== "object") obj[key] = {};
  return obj[key];
}

function splitPin(s) {
  const i = s.indexOf(".");
  return [s.slice(0, i), s.slice(i + 1)];
}

/** Reads KEY=VALUE lines from a .env file into process.env (existing values win). */
function loadDotEnv(file) {
  let text;
  try {
    text = fs.readFileSync(file, "utf8");
  } catch {
    return;
  }
  for (const line of text.split(/\r?\n/)) {
    const m = /^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$/.exec(line);
    if (!m || m[1] in process.env) continue;
    process.env[m[1]] = dotEnvValue(m[2]);
  }
}

/**
 * A .env value: inside quotes as written; otherwise trimmed, with a comment
 * after ` #` left out (`TOKEN=abc # mine` is `abc`).
 */
function dotEnvValue(raw) {
  const quoted = /^\s*(['"])(.*?)\1/.exec(raw);
  if (quoted) return quoted[2];
  return raw.replace(/(^|\s)#.*$/, "").trim();
}

function cli() {
  const args = process.argv.slice(2);
  const arg = (name) => {
    const i = args.indexOf(name);
    return i >= 0 ? args[i + 1] : undefined;
  };
  const bundlePath = arg("--bundle");
  if (!bundlePath) {
    console.error("usage: node circuit-runtime.js --bundle bundle.json --deps <dir> [--data <dir>] [--debug]");
    process.exit(2);
  }
  const bundle = JSON.parse(fs.readFileSync(bundlePath, "utf8"));
  if (args.includes("--stop-on-input")) listenForStop();
  start(bundle, {
    debug: args.includes("--debug"),
    depsRoot: path.resolve(arg("--deps") || process.cwd()),
    dataDir: path.resolve(arg("--data") || path.join(process.cwd(), "data")),
  });
}

function reportFatal(text) {
  if (process.argv.includes("--debug")) process.stdout.write(PROTOCOL + JSON.stringify({ t: "log", level: "error", text }) + "\n");
  else console.error(text);
}

process.on("unhandledRejection", (e) => {
  reportFatal(`Unhandled error: ${e && e.stack ? e.stack : e}`);
});

// Pending saved data is written however the bot ends.
process.on("exit", () => {
  for (const s of openStores) s.flushPending();
});
process.on("uncaughtException", (e) => {
  reportFatal(`Bot crashed: ${e && e.stack ? e.stack : e}`);
  process.exit(1);
});
for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => process.exit(0));

/** XC Circuit stops a running bot by writing "stop" to it (or by closing its input). */
function listenForStop() {
  process.stdin.setEncoding("utf8");
  let buffer = "";
  process.stdin.on("data", (chunk) => {
    buffer += chunk;
    const lines = buffer.split(/\r?\n/);
    buffer = lines.pop();
    for (const line of lines.map((l) => l.trim())) {
      if (line === "stop") process.exit(0);
      // "answer <id> <value>": XC Circuit's reply to an ask event.
      const m = /^answer (\S+) (.*)$/.exec(line);
      if (m && pendingAnswers.has(m[1])) {
        const resolve = pendingAnswers.get(m[1]);
        pendingAnswers.delete(m[1]);
        resolve(m[2]);
      }
    }
  });
  process.stdin.on("end", () => process.exit(0));
  process.stdin.on("error", () => {});
}

if (typeof __XC_BUNDLE__ !== "undefined") {
  loadDotEnv(path.join(__dirname, ".env"));
  start(__XC_BUNDLE__, { debug: process.argv.includes("--debug"), depsRoot: __dirname, dataDir: path.join(__dirname, "data") });
} else if (require.main === module) {
  cli();
}

module.exports = { start, ungroupBlock, fillTemplate, expressionEnd, coerce, Store, backupStore, listStoreBackups, prepareStoreFile, BACKUPS_KEPT };
