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
    rt.log.error(`Bot stopped: ${e && e.message ? e.message : e}${errorDetails(e)}`);
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
    this.conversions = new Map(); // pin type -> ways to turn other types into it
    this.fromIds = new Map(); // pin type -> (id, ctx) => its value with that id
    this.typeParents = new Map(); // pin type -> the type it extends
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
    /** The connections of Bot › Settings › Databases, opened when first used. */
    this.databases = new Databases(this);
    /** One-time timers (Schedule, On Scheduled), kept in the stored data. */
    this.timers = new Timers(this);
    runtimes.add(this);
    this.runCounter = 0;
    this.log = makeLogger(this, null);
  }

  emit(type, data) {
    if (this.debug) process.stdout.write(PROTOCOL + JSON.stringify({ t: type, ...data }) + "\n");
  }

  secret(name) {
    const env = process.env[`XC_SECRET_${String(name).toUpperCase()}`];
    if (env) return env;
    // An exported bot's token, in the variable its mode names (FLUXER_BOT_TOKEN).
    const own = this.bundle.project.tokenEnv;
    if (name === "token") return own && own !== "DISCORD_TOKEN" ? process.env[own] : process.env.DISCORD_TOKEN || process.env.DBB_TOKEN;
    return undefined;
  }

  setting(name) {
    return (this.bundle.project.settings || {})[name];
  }

  /**
   * A file a block brought along (`requires.assets`): written once into the
   * data folder's .assets, and its path given back.
   */
  asset(blockId, name) {
    const data = ((this.bundle.blocks[blockId] || {}).assets || {})[name];
    if (data == null) throw new Error(`${blockId} has no file "${name}": list it in requires.assets`);
    this.assetFiles = this.assetFiles || new Map();
    const key = `${blockId}\u0000${name}`;
    if (!this.assetFiles.has(key)) {
      const file = path.join(this.opts.dataDir || process.cwd(), ".assets", blockId.replace(/[^\w.-]/g, "_"), path.basename(name));
      const bytes = Buffer.from(data, "base64");
      let same = false;
      try {
        same = fs.readFileSync(file).equals(bytes);
      } catch {}
      if (!same) {
        fs.mkdirSync(path.dirname(file), { recursive: true });
        fs.writeFileSync(file, bytes);
      }
      this.assetFiles.set(key, file);
    }
    return this.assetFiles.get(key);
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

  /**
   * A wired value as its input's type wants it: a Member wired into a User
   * input becomes the member's user (one by one in lists).
   */
  /** How to find a value of `type` (or a type it extends) by its id, or null. */
  fromIdFor(type) {
    let t = String(type || "").replace(/\?$/, "");
    for (let i = 0; i < 16 && t; i++) {
      if (this.fromIds.has(t)) return this.fromIds.get(t);
      t = this.typeParents.get(t);
    }
    return null;
  }

  convert(value, type) {
    if (value == null || !this.conversions.size) return value;
    const target = String(type || "").replace(/\?$/, "");
    const list = /^list<(.+)>$/.exec(target);
    const ways = this.conversions.get(list ? list[1] : target);
    if (!ways) return value;
    const one = (v) => {
      for (const w of ways) {
        try {
          if (w.test(v)) return w.via(v);
        } catch {}
      }
      return v;
    };
    return list && Array.isArray(value) ? value.map(one) : one(value);
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
    const storeIn = this.setting("storeIn");
    if (storeIn && storeIn !== "json") {
      // Stored values in a database (Bot › Settings › Databases).
      this.store = await DbStore.open(this, storeIn);
    } else {
      // First: check the saved data, back it up, and ask if it is empty.
      await prepareStoreFile(this.opts.dataDir, this);
      this.store = new Store(this.opts.dataDir, this);
      this.backupTimer = setInterval(() => backupStore(this.opts.dataDir, this, true), BACKUP_EVERY_MS);
      this.backupTimer.unref?.();
    }
    if (this.store.note) this.log.warn(this.store.note);
    this.log.info(`Runtime ready · Node ${process.version}`);
    this.conversions = compileConversions(this.bundle.types, this.log);
    for (const [id, t] of Object.entries(this.bundle.types || {})) {
      if (t.extends) this.typeParents.set(id, t.extends);
      if (!t.fromId) continue;
      try {
        const fn = (0, eval)(`(${t.fromId})`);
        if (typeof fn === "function") this.fromIds.set(id, fn);
        else this.log.warn(`Pin type ${id} fromId isn't a function`);
      } catch (e) {
        this.log.warn(`Pin type ${id} fromId: ${e.message}`);
      }
    }
    for (const [id, b] of Object.entries(this.bundle.blocks)) {
      try {
        this.blocks.set(id, ungroupBlock(this.loadModule(esmToCommonJs(b.source), b.file)));
      } catch (e) {
        this.log.error(`Could not load block ${id}: ${e.message}${errorDetails(e)}`);
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
        this.log.error(`Runtime library ${lib.file} failed: ${e.message}${errorDetails(e)}`);
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
        this.log.error(`${e.message}${errorDetails(e)}`);
      }
    }
    for (const h of this.startHandlers) h({});
    await this.timers.start();
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
      /** This block, as `workspace:node`: a key for data of its own (what it has seen …). */
      id: `${graph.key}:${node.id}`,
      fields: node.inline(),
      /**
       * What the bot's author typed into field `key`, before ${…} was filled;
       * undefined when the field is wired. A typed value without ${…} is the
       * author's own; anything else may come from what people type.
       */
      typed: (key) => (graph.inputs.has(`${node.id}.${key}`) ? undefined : node.inline()[key]),
      /** Whether field `key` is exactly what the author typed: not wired, no ${…}. */
      authored: (key) => {
        if (graph.inputs.has(`${node.id}.${key}`)) return false;
        const v = node.inline()[key];
        return typeof v !== "string" || !v.includes("${");
      },
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
        stop: () => shutdown(0),
        restart: () => shutdown(75),
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
        /** A secret of Bot › Settings (an API key a plugin's page asks for). */
        secret: (name) => rt.secret(name),
        /** The bot's data folder (stored data, databases, files). */
        dataDir: rt.opts.dataDir || process.cwd(),
      },
      util: UTIL,
      /** A file the block lists in `requires.assets` (a font …), as a path on disk. */
      asset: (name) => rt.asset(node.blockId, name),
      /** One-time timers: schedule, cancel, list, and on(name, fn) to run when one is due. */
      timers: rt.timers.api(),
      /**
       * A database of Bot › Settings › Databases: its id or name, or "default".
       * Or one the block gives itself: { kind, host, port, user, password,
       * database, ssl, url }, once Bot › Settings › Databases allows that.
       */
      db: (which) => rt.databases.get(which),
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
      this.log.error(`${where}: ${details.message}${fromOnError ? " (in an On Error path)" : ""}${errorDetails(err, details.message)}`);
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
  requires: ["intents", "dependencies", "permissions"],
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
        this.rt.log.error(`${this.name} › ${node.def.name}: listen() failed: ${e.message}${errorDetails(e)}`);
      }
    }
  }

  trigger(node, outputs) {
    const run = new Run(this, outputs);
    run.setOutputs(node.id, outputs);
    this.rt.emit("node", { ws: this.key, node: node.id, phase: "enter", run: run.id });
    this.rt.emit("node", { ws: this.key, node: node.id, phase: "exit", run: run.id, ms: 0, outputs: this.rt.debug ? previews(outputs) : undefined });
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
    this.rt.emit("node", { ws: this.key, node: node.id, phase: "enter", run: run.id });
    try {
      // A zone that timed out stops at its next block.
      if (path.signal && path.signal.aborted) throw new Error("timed out");
      const fields = await node.resolve(run, new Map(), path);
      const ctx = this.rt.ctxFor(run, node, path);
      // Its own time, without the value blocks that worked out its inputs.
      const t0 = performance.now();
      const result = await node.def.run({ fields, ctx, node: { id: node.id, workspace: this.key } });
      if (result && typeof result === "object") run.setOutputs(node.id, result);
      this.rt.emit("node", { ws: this.key, node: node.id, phase: "exit", run: run.id, ms: +(performance.now() - t0).toFixed(1), outputs: this.rt.debug ? previews(run.outputs.get(node.id)) : undefined });
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
    this.rt.emit("zone", { ws: this.key, zone: zoneId, phase: "error", run: run.id, error: details.message, outputs: this.rt.debug ? previews({ message: details.message, details }) : undefined });
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
          // Value blocks report their runs too, so the editor shows them glow and how long they took.
          this.rt.emit("node", { ws: this.key, node: node.id, phase: "enter", run: run.id });
          const t0 = performance.now();
          const out = (await node.def.run({ fields, ctx: this.rt.ctxFor(run, node, path), node: { id: node.id, workspace: this.key } })) || {};
          this.rt.emit("node", { ws: this.key, node: node.id, phase: "exit", run: run.id, ms: +(performance.now() - t0).toFixed(1), outputs: this.rt.debug ? previews(out) : undefined });
          return out;
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
      if (def) fields[pin] = this.graph.rt.convert(fields[pin], def.type);
      wired.add(pin);
    }
    // Values wired into a text field's {name} inputs (pins that aren't fields).
    const placed = {};
    for (const pin of wired) if (!(pin in (this.def.fields || {}))) placed[pin] = fields[pin];
    for (const [k, f] of Object.entries(this.def.fields || {})) {
      if (!f) continue;
      if (f.templateParts && !wired.has(k) && typeof fields[k] === "string") {
        // Kept apart, like a tagged template: the database blocks send the values as parameters.
        fields[k] = templateParts(fields[k], run, fields, placed);
        continue;
      }
      if (templated(f) && !wired.has(k) && typeof fields[k] === "string") fields[k] = fillTemplate(fields[k], run, fields, placed);
      fields[k] = coerce(fields[k], f.type);
      await this.findById(fields, k, f, run, path);
    }
    // Inputs the block adds in define() (Send Message's channel): ids typed
    // into them are looked up too. Their values are otherwise left as they are.
    for (const [k, f] of Object.entries(this.addedFields())) {
      if (f && !(k in (this.def.fields || {}))) await this.findById(fields, k, f, run, path);
    }
    return fields;
  }

  /**
   * An id (typed, or a text wired in) where a value of a type with `fromId`
   * goes (a channel): the value itself. An empty id is nothing, as an
   * unconnected pin is.
   */
  async findById(fields, k, f, run, path) {
    const value = fields[k];
    if (typeof value !== "string" && typeof value !== "number") return;
    const find = this.graph.rt.fromIdFor(f.type);
    if (!find) return;
    if (!String(value).trim()) {
      fields[k] = null;
      return;
    }
    const id = String(value).trim().replace(/^<[#@&!]*|>$/g, "");
    const ctx = this.graph.rt.ctxFor(run, this, path);
    const found = /^[\w-]+$/.test(id) ? await Promise.resolve().then(() => find(id, ctx)).catch(() => null) : null;
    if (!found) throw new Error(`${f.label || k}: no ${String(f.type).replace(/\?$/, "").split(".").pop().replace(/_/g, " ")} with the id "${value}"`);
    fields[k] = found;
  }

  /**
   * The inputs the block's define() adds for this block's values, as the
   * editor shows them; worked out once. None when it has no define() or it fails.
   */
  addedFields() {
    if (this.added) return this.added;
    this.added = {};
    if (typeof this.def.define !== "function") return this.added;
    try {
      const result = this.def.define({ values: this.inline(), connected: {}, project: this.graph.rt.bundle.project || {} });
      if (result && typeof result.add === "object") this.added = result.add;
    } catch {
      // The editor showed the problem already; the block runs with what it has.
    }
    return this.added;
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

/**
 * Like fillTemplate, but keeps the texts and the values of the ${…} apart:
 * { strings, values }, with one more string than values (as a tagged
 * template gets them). SQL Query sends the values as parameters.
 */
function templateParts(text, run, fields, placed = {}) {
  const strings = [];
  const values = [];
  let from = 0;
  for (;;) {
    const open = text.indexOf("${", from);
    const close = open < 0 ? -1 : expressionEnd(text, open + 2);
    if (close < 0) {
      strings.push(text.slice(from));
      return { strings, values };
    }
    const expr = text.slice(open + 2, close);
    strings.push(text.slice(from, open));
    try {
      values.push(evaluate(expr, run, fields, placed));
    } catch (e) {
      throw new Error(`\${${expr}}: ${e.message}`);
    }
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

/**
 * Pin type conversions (`[[type.converts]]`) by target type: each with the
 * source type's `check`, to recognise its values, and `via`, to convert them.
 */
function compileConversions(types, log) {
  const compile = (source, what) => {
    try {
      const fn = (0, eval)(`(${source})`);
      if (typeof fn === "function") return fn;
      log.warn(`Pin type ${what} isn't a function`);
    } catch (e) {
      log.warn(`Pin type ${what}: ${e.message}`);
    }
    return null;
  };
  const byTarget = new Map();
  for (const [id, t] of Object.entries(types || {})) {
    if (!t.check || !(t.converts || []).length) continue;
    const test = compile(t.check, `${id} check`);
    if (!test) continue;
    for (const c of t.converts) {
      const via = compile(c.via, `${id} → ${c.to}`);
      if (!via) continue;
      if (!byTarget.has(c.to)) byTarget.set(c.to, []);
      byTarget.get(c.to).push({ from: id, test, via });
    }
  }
  return byTarget;
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
    message: explainError(e, node) || String(e.message || e),
    code: e.code ?? e.status ?? null,
    block: node ? node.def.name || node.blockId : null,
    node: node ? node.id : null,
    workspace: node ? node.graph.name : null,
    stack: e.stack || null,
  };
}

/**
 * The rest of an error after its message, for the log: the original message
 * when a clearer one is shown, what kind of error it is (name, code, HTTP
 * status and request), where it happened (the stack) and what caused it.
 * Each on a line of its own, starting with a line break; "" when there's
 * nothing more to say.
 */
function errorDetails(err, shown) {
  if (!err || typeof err !== "object") return "";
  const lines = [];
  const message = String(err.message ?? "");
  if (shown != null && message && message !== shown) lines.push(`Original error: ${message}`);
  const facts = [];
  if (err.name && err.name !== "Error") facts.push(String(err.name));
  if (err.code != null) facts.push(`code ${err.code}`);
  const status = err.status ?? err.httpStatus;
  if (status != null && status !== err.code) facts.push(`status ${status}`);
  if (err.method && err.url) facts.push(`${String(err.method).toUpperCase()} ${err.url}`);
  if (facts.length) lines.push(facts.join(" · "));
  // The stack without its first line(s), which repeat the message.
  const stack = String(err.stack || "")
    .split("\n")
    .filter((l) => /^\s+at /.test(l))
    .map((l) => `  ${l.trim()}`);
  lines.push(...stack);
  let cause = err.cause;
  for (let depth = 0; cause && depth < 3; depth++) {
    lines.push(`Caused by: ${cause.message ?? cause}`);
    if (typeof cause !== "object") break;
    lines.push(...String(cause.stack || "").split("\n").filter((l) => /^\s+at /.test(l)).map((l) => `  ${l.trim()}`));
    cause = cause.cause;
  }
  return lines.length ? `\n${lines.join("\n")}` : "";
}

/**
 * A clearer message from the bot's library (`explainError(err, def, values)`),
 * such as which permission the bot is missing; null when none has one.
 */
function explainError(err, node) {
  const rt = node?.graph?.rt;
  if (!rt) return null;
  for (const api of rt.libs.values()) {
    if (typeof api?.explainError !== "function") continue;
    try {
      const better = api.explainError(err, node.def, node.inline());
      if (better) return better;
    } catch {}
  }
  return null;
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
    .sort((a, b) => compareBackupNames(b.name, a.name));
}

/**
 * Orders backup names by when they were made: by their time stamp, then by
 * the counter added when two were made in the same millisecond
 * (store-<stamp>.json, store-<stamp>-2.json, …).
 */
function compareBackupNames(a, b) {
  const parse = (name) => {
    const m = /^store-(.*Z)(?:-(\d+))?\.json$/.exec(name);
    return m ? [m[1], Number(m[2] || 1)] : [name, 0];
  };
  const [sa, na] = parse(a);
  const [sb, nb] = parse(b);
  if (sa !== sb) return sa < sb ? -1 : 1;
  return na - nb;
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
    // Made in the same millisecond as others: number it after the highest
    // (not into a gap left by removed ones, which would sort as older).
    const same = fs.readdirSync(backups).filter((n) => n === `store-${stamp}.json` || n.startsWith(`store-${stamp}-`));
    const highest = Math.max(0, ...same.map((n) => Number((/-(\d+)\.json$/.exec(n.slice(`store-${stamp}`.length)) || [0, 1])[1])));
    const target = path.join(backups, highest ? `store-${stamp}-${highest + 1}.json` : `store-${stamp}.json`);
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

  /** Value `name` of bucket `bucket` changed (or was deleted). */
  changed(bucket, name) {
    this.save();
  }

  /** Writes what's waiting, for a bot that is about to end. */
  async flushAll() {
    this.flushPending();
  }

  scope(...parts) {
    const key = storeKey(parts);
    const bucket = () => getOrObj(this.data, key);
    return {
      get: async (k, fallback) => (k in bucket() ? bucket()[k] : fallback),
      has: async (k) => k in bucket(),
      set: async (k, v) => {
        bucket()[k] = v;
        this.changed(key, k);
      },
      delete: async (k) => {
        delete bucket()[k];
        this.changed(key, k);
      },
      inc: async (k, by = 1) => {
        const b = bucket();
        b[k] = (Number(b[k]) || 0) + Number(by);
        this.changed(key, k);
        return b[k];
      },
      push: async (k, v) => {
        const b = bucket();
        b[k] = Array.isArray(b[k]) ? [...b[k], v] : [v];
        this.changed(key, k);
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

// ─── Databases ───────────────────────────────────────────────────────────────
// The connections of Bot › Settings › Databases: MariaDB/MySQL (mysql2),
// PostgreSQL (pg), SQLite (built into Node.js) and MongoDB (mongodb). The
// editor adds the npm package a connection needs to the bot's packages.
// Values always go to the database as parameters, never into the SQL text.

/** Every Runtime, so a bot that ends can close its connections. */
const runtimes = new Set();

const DB_KINDS = {
  mariadb: { name: "MariaDB / MySQL", port: 3306, package: "mysql2" },
  postgres: { name: "PostgreSQL", port: 5432, package: "pg" },
  sqlite: { name: "SQLite" },
  mongodb: { name: "MongoDB", port: 27017, package: "mongodb" },
};

/** Comparisons of a condition: { column, op, value }. */
const SQL_OPS = { "=": "=", "!=": "<>", ">": ">", ">=": ">=", "<": "<", "<=": "<=" };
const LIKE_OPS = { contains: (v) => `%${v}%`, starts: (v) => `${v}%`, ends: (v) => `%${v}` };
const MONGO_OPS = { "!=": "$ne", ">": "$gt", ">=": "$gte", "<": "$lt", "<=": "$lte" };

const NAME = /^[A-Za-z_][A-Za-z0-9_]*$/;

/** Databases given on blocks kept open at once. */
const MAX_GIVEN = 8;

/** The table where Set/Get Stored Value keep their values, when they use a database. */
const STORE_TABLE = "xc_store";

class Databases {
  constructor(rt) {
    this.rt = rt;
    this.open = new Map(); // id -> Database
    this.givenOpen = []; // databases given on blocks, the least recently used first
    this.givenCount = 0;
  }

  /** The connections in Bot › Settings › Databases. */
  list() {
    const list = this.rt.setting("databases");
    return Array.isArray(list) ? list.filter((d) => d && d.id) : [];
  }

  /** The database `which` names (its id or name), or the default one for "default" or nothing; an object gives the connection itself. */
  get(which) {
    if (which && typeof which === "object") return this.given(which);
    const list = this.list();
    let spec;
    if (!which || which === "default") {
      const id = this.rt.setting("databaseDefault");
      spec = list.find((d) => d.id === id) || (list.length === 1 ? list[0] : undefined);
      if (!spec) {
        throw new Error(list.length ? "No default database is chosen. Choose one in Bot › Settings › Databases, or pick a database on the block." : "No database is set up yet. Add one in Bot › Settings › Databases.");
      }
    } else {
      spec = list.find((d) => d.id === which) || list.find((d) => d.name === which);
      if (!spec) throw new Error(`There's no database "${which}" in Bot › Settings › Databases.`);
    }
    return getOr(this.open, spec.id, () => new Database(spec, this.rt));
  }

  /**
   * A database a block gives itself (from a command's options, say), when
   * Bot › Settings › Databases allows that. It stays open for the next run
   * with the same details; the least recently used closes past MAX_GIVEN.
   */
  given(spec) {
    if (this.rt.setting("databaseGivenAllowed") !== true) {
      throw new Error("Databases given on a block are off: allow them in Bot › Settings › Databases › Safety");
    }
    const kind = String(spec.kind || "mariadb");
    if (!DB_KINDS[kind] || kind === "sqlite") throw new Error(`A database given on a block is MariaDB / MySQL, PostgreSQL or MongoDB, not "${kind}".`);
    const text = (v) => (v === undefined || v === null ? "" : String(v).trim());
    const s = {
      kind,
      host: text(spec.host),
      port: Number(spec.port) || undefined,
      user: text(spec.user),
      database: text(spec.database),
      ssl: spec.ssl === true || spec.ssl === "true",
      password: spec.password === undefined || spec.password === null ? "" : String(spec.password),
      url: text(spec.url),
    };
    if (!s.host && !s.url) throw new Error("The block gives no host and no connection URL for its database.");
    // The same details: the same connection. Compared as they are (no key
    // made from them: a password is never hashed or written anywhere).
    const same = (d) => Object.keys(s).every((k) => d.spec[k] === s[k]);
    const at = this.givenOpen.findIndex(same);
    let db;
    if (at >= 0) {
      [db] = this.givenOpen.splice(at, 1); // to the end: used most recently
    } else {
      while (this.givenOpen.length >= MAX_GIVEN) this.givenOpen.shift().close().catch(() => {});
      // Its name in messages: never the password or the URL (which can hold it).
      const name = s.host ? `${s.host}${s.port ? `:${s.port}` : ""}` : "the given database";
      db = new Database({ ...s, id: `given-${++this.givenCount}`, name, given: true }, this.rt);
    }
    this.givenOpen.push(db);
    return db;
  }

  async closeAll() {
    const all = [...this.open.values(), ...this.givenOpen];
    this.open.clear();
    this.givenOpen = [];
    await Promise.allSettled(all.map((d) => d.close()));
  }
}

class Database {
  constructor(spec, rt) {
    this.spec = spec;
    this.rt = rt;
    this.kind = spec.kind;
    this.name = spec.name || spec.id;
    if (!DB_KINDS[this.kind]) throw new Error(`${this.name}: "${spec.kind}" isn't a kind of database XC Circuit knows.`);
    this.client = null;
    this.connecting = null;
  }

  /** Whether it speaks SQL (everything but MongoDB). */
  get sql() {
    return this.kind !== "mongodb";
  }

  /** The connection's password or URL, from the system keyring (or .env in an exported bot), or as the block gave it. */
  secret(part) {
    if (this.spec.given) return this.spec[part] || undefined;
    return this.rt.secret(`db_${this.spec.id}_${part}`);
  }

  /** Connects once; later calls wait for the same connection. */
  connect() {
    if (!this.connecting) {
      this.connecting = this.open().catch((e) => {
        this.connecting = null;
        throw this.explain(e);
      });
    }
    return this.connecting;
  }

  require(name) {
    try {
      return this.rt.require(name);
    } catch (e) {
      if (e && e.code === "MODULE_NOT_FOUND") {
        throw new Error(`the npm package ${name} isn't installed. XC Circuit installs it when you run the bot; an exported bot needs npm install once.`);
      }
      throw e;
    }
  }

  async open() {
    const s = this.spec;
    const url = this.secret("url");
    const password = this.secret("password");
    const port = Number(s.port) || DB_KINDS[this.kind].port;
    switch (this.kind) {
      case "mariadb": {
        const mysql = this.require("mysql2/promise");
        // Big numbers (such as Discord ids in BIGINT columns) come back as text when a number can't hold them exactly.
        const common = { connectionLimit: 5, supportBigNumbers: true, dateStrings: false };
        this.client = url
          ? mysql.createPool({ uri: url, ...common })
          : mysql.createPool({ host: s.host || "localhost", port, user: s.user || undefined, password, database: s.database || undefined, ssl: s.ssl ? {} : undefined, ...common });
        await this.client.query("SELECT 1");
        break;
      }
      case "postgres": {
        const pg = this.require("pg");
        // BIGINT as a number when it fits exactly, otherwise as text (like Discord ids).
        pg.types.setTypeParser(20, (v) => (Number.isSafeInteger(Number(v)) ? Number(v) : v));
        this.client = url
          ? new pg.Pool({ connectionString: url, max: 5 })
          : new pg.Pool({ host: s.host || "localhost", port, user: s.user || undefined, password, database: s.database || undefined, ssl: s.ssl ? true : undefined, max: 5 });
        await this.client.query("SELECT 1");
        break;
      }
      case "sqlite": {
        const file = path.resolve(this.rt.opts.dataDir || process.cwd(), s.file || "bot.sqlite");
        fs.mkdirSync(path.dirname(file), { recursive: true });
        let sqlite;
        // node:sqlite says it's experimental on every start; that's no news for bot makers.
        const warn = process.emitWarning;
        process.emitWarning = (w, ...rest) => (String(w && w.message ? w.message : w).includes("SQLite") ? undefined : warn.call(process, w, ...rest));
        try {
          sqlite = require("node:sqlite");
        } catch {
          throw new Error(`SQLite needs Node.js 22.13 or newer; this is ${process.version}.`);
        } finally {
          process.emitWarning = warn;
        }
        this.client = new sqlite.DatabaseSync(file);
        this.file = file;
        break;
      }
      case "mongodb": {
        const { MongoClient, ObjectId } = this.require("mongodb");
        this.ObjectId = ObjectId;
        const auth = s.user ? `${encodeURIComponent(s.user)}:${encodeURIComponent(password || "")}@` : "";
        this.client = new MongoClient(url || `mongodb://${auth}${s.host || "localhost"}:${port}/`, { serverSelectionTimeoutMS: 8000 });
        await this.client.connect();
        this.db = this.client.db(s.database || undefined);
        break;
      }
    }
  }

  /** An error that says which database, and what to check. */
  explain(e) {
    const s = this.spec;
    const where = s.host ? `${s.host}:${Number(s.port) || DB_KINDS[this.kind].port}` : "";
    const code = e && (e.code || (e.cause && e.cause.code));
    let text = e && e.message ? e.message : String(e);
    if (code === "ECONNREFUSED") text = `nothing answers at ${where || "its address"}. Is the database running, and is the port right?`;
    else if (code === "ENOTFOUND" || code === "EAI_AGAIN") text = `the host ${s.host} wasn't found. Check its name.`;
    else if (code === "ETIMEDOUT" || (e && e.name === "MongoServerSelectionError")) text = `${where || "the database"} didn't answer in time. Is it reachable from here (firewall, allowed IPs)?`;
    else if (code === "ER_ACCESS_DENIED_ERROR" || code === "28P01" || (e && e.codeName === "AuthenticationFailed")) text = "the user name or password is wrong.";
    else if (code === "ER_BAD_DB_ERROR" || code === "3D000") text = `there's no database called "${s.database}" on the server.`;
    const err = new Error(`${this.name}: ${text}`);
    err.cause = e;
    return err;
  }

  /** A table or column name, checked and quoted. `${…}` values can't be names. */
  ident(name) {
    const parts = String(name ?? "").trim().split(".");
    if (!parts.every((p) => NAME.test(p))) {
      throw new Error(`"${name}" can't be a table or column name here: use letters, digits and _ (and a . between a schema and a table).`);
    }
    const q = this.kind === "mariadb" ? "`" : '"';
    return parts.map((p) => q + p + q).join(".");
  }

  /** Placeholder number `i` (1-based). */
  ph(i) {
    return this.kind === "postgres" ? `$${i}` : "?";
  }

  /** A value as the driver takes it. */
  value(v) {
    if (v === undefined || v === null) return null;
    if (typeof v === "number" || typeof v === "string" || typeof v === "bigint") return v;
    if (typeof v === "boolean") return this.kind === "sqlite" ? (v ? 1 : 0) : v;
    if (v instanceof Date) return this.kind === "sqlite" ? v.toISOString() : v;
    if (Buffer.isBuffer(v)) return v;
    if (isEntity(v)) return v.id;
    if (!this.sql) return v;
    return JSON.stringify(v);
  }

  /** Runs SQL with its parameters: { rows, affected, id }. */
  async run(sql, params = []) {
    if (!this.sql) throw new Error(`${this.name} is MongoDB: SQL works with MariaDB, MySQL, PostgreSQL and SQLite. Use Get Rows, Insert Row, Update Rows and Delete Rows.`);
    await this.connect();
    try {
      switch (this.kind) {
        case "mariadb": {
          const [r] = await this.client.query(sql, params);
          if (Array.isArray(r)) return { rows: r.map(plain), affected: 0 };
          return { rows: [], affected: r.affectedRows || 0, id: r.insertId || undefined };
        }
        case "postgres": {
          const r = await this.client.query(sql, params);
          return { rows: (r.rows || []).map(plain), affected: r.rowCount || 0 };
        }
        case "sqlite": {
          const st = this.client.prepare(sql);
          if (/^\s*(select|with|pragma|explain|values)\b/i.test(sql) || /\breturning\b/i.test(sql)) return { rows: st.all(...params).map(plain), affected: 0 };
          const r = st.run(...params);
          return { rows: [], affected: Number(r.changes) || 0, id: r.lastInsertRowid ? Number(r.lastInsertRowid) : undefined };
        }
      }
    } catch (e) {
      throw new Error(`${this.name}: ${e.message}`, { cause: e });
    }
  }

  /** SQL with ${…} values ({ strings, values }, or plain text) run as parameters. */
  async query(parts) {
    if (typeof parts === "string") return this.run(parts);
    const params = [];
    let sql = parts.strings[0];
    for (let i = 0; i < parts.values.length; i++) {
      params.push(this.value(parts.values[i]));
      sql += this.ph(params.length) + parts.strings[i + 1];
    }
    return this.run(sql, params);
  }

  /** WHERE for conditions [{ column, op, value }], all of them or (`any`) one. */
  where(conds, params, any) {
    const parts = [];
    for (const c of conds || []) {
      const col = this.ident(c.column);
      const op = c.op || "=";
      if (op === "null" || ((op === "=" || op === "!=") && c.value == null)) {
        parts.push(`${col} IS ${op === "!=" ? "NOT " : ""}NULL`);
      } else if (op === "notNull") {
        parts.push(`${col} IS NOT NULL`);
      } else if (LIKE_OPS[op]) {
        params.push(LIKE_OPS[op](String(c.value ?? "").replace(/[!%_]/g, "!$&")));
        // Upper and lower case count as the same everywhere.
        const left = this.kind === "postgres" ? `${col}::text ILIKE` : `${col} LIKE`;
        parts.push(`${left} ${this.ph(params.length)} ESCAPE '!'`);
      } else if (SQL_OPS[op]) {
        params.push(this.value(c.value));
        parts.push(`${col} ${SQL_OPS[op]} ${this.ph(params.length)}`);
      } else {
        throw new Error(`"${op}" isn't a way to compare.`);
      }
    }
    return parts.length ? ` WHERE ${parts.join(any ? " OR " : " AND ")}` : "";
  }

  limit(limit, offset) {
    const n = Math.floor(Number(limit) || 0);
    const skip = Math.floor(Number(offset) || 0);
    if (n > 0) return ` LIMIT ${n}${skip > 0 ? ` OFFSET ${skip}` : ""}`;
    if (skip <= 0) return "";
    if (this.kind === "postgres") return ` OFFSET ${skip}`;
    return ` LIMIT ${this.kind === "sqlite" ? -1 : "18446744073709551615"} OFFSET ${skip}`;
  }

  /** The rows of `table` that match: [{ column: value }]. */
  async select(table, where = [], { order, desc, limit, offset, any } = {}) {
    if (!this.sql) {
      await this.connect();
      const opts = {};
      if (order) opts.sort = { [mongoName(order)]: desc ? -1 : 1 };
      if (Number(offset) > 0) opts.skip = Math.floor(Number(offset));
      if (Number(limit) > 0) opts.limit = Math.floor(Number(limit));
      return this.mongo(async () => (await this.db.collection(mongoName(table)).find(this.filter(where, any), opts).toArray()).map(mongoPlain));
    }
    const params = [];
    let sql = `SELECT * FROM ${this.ident(table)}${this.where(where, params, any)}`;
    if (order) sql += ` ORDER BY ${this.ident(order)}${desc ? " DESC" : ""}`;
    sql += this.limit(limit, offset);
    return (await this.run(sql, params)).rows;
  }

  /** Adds a row: { id, row }. `row` is the new row where the database gives it back. */
  async insert(table, values) {
    const cols = Object.keys(values || {});
    if (!cols.length) throw new Error("There's nothing to insert: give at least one column a value.");
    if (!this.sql) {
      await this.connect();
      const doc = Object.fromEntries(cols.map((c) => [mongoName(c), this.value(values[c])]));
      const r = await this.mongo(() => this.db.collection(mongoName(table)).insertOne(doc));
      return { id: String(r.insertedId), row: mongoPlain({ ...doc, _id: r.insertedId }) };
    }
    const params = cols.map((c) => this.value(values[c]));
    let sql = `INSERT INTO ${this.ident(table)} (${cols.map((c) => this.ident(c)).join(", ")}) VALUES (${params.map((_, i) => this.ph(i + 1)).join(", ")})`;
    if (this.kind !== "mariadb") sql += " RETURNING *";
    const r = await this.run(sql, params);
    const row = r.rows[0];
    return { id: r.id ?? (row ? row.id : undefined), row };
  }

  /** Changes the matching rows; gives how many. */
  async update(table, values, where = [], any = false) {
    const cols = Object.keys(values || {});
    if (!cols.length) throw new Error("There's nothing to change: give at least one column a value.");
    if (!this.sql) {
      await this.connect();
      const set = Object.fromEntries(cols.map((c) => [mongoName(c), this.value(values[c])]));
      const r = await this.mongo(() => this.db.collection(mongoName(table)).updateMany(this.filter(where, any), { $set: set }));
      return r.modifiedCount;
    }
    const params = [];
    const set = cols.map((c) => {
      params.push(this.value(values[c]));
      return `${this.ident(c)} = ${this.ph(params.length)}`;
    });
    return (await this.run(`UPDATE ${this.ident(table)} SET ${set.join(", ")}${this.where(where, params, any)}`, params)).affected;
  }

  /** Deletes the matching rows; gives how many. */
  async remove(table, where = [], any = false) {
    if (!this.sql) {
      await this.connect();
      const r = await this.mongo(() => this.db.collection(mongoName(table)).deleteMany(this.filter(where, any)));
      return r.deletedCount;
    }
    const params = [];
    return (await this.run(`DELETE FROM ${this.ident(table)}${this.where(where, params, any)}`, params)).affected;
  }

  /** A MongoDB filter for conditions. Values are compared as values ($eq), never read as operators. */
  filter(conds, any) {
    const parts = (conds || []).map((c) => {
      const col = mongoName(c.column);
      const op = c.op || "=";
      let v = this.value(c.value);
      if (col === "_id" && typeof v === "string" && /^[0-9a-f]{24}$/i.test(v)) v = new this.ObjectId(v);
      if (op === "null") return { [col]: null };
      if (op === "notNull") return { [col]: { $ne: null } };
      if (LIKE_OPS[op]) {
        const text = String(c.value ?? "").replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
        return { [col]: { $regex: op === "starts" ? `^${text}` : op === "ends" ? `${text}$` : text, $options: "i" } };
      }
      if (op === "=") return { [col]: { $eq: v } };
      if (MONGO_OPS[op]) return { [col]: { [MONGO_OPS[op]]: v } };
      throw new Error(`"${op}" isn't a way to compare.`);
    });
    if (!parts.length) return {};
    if (parts.length === 1) return parts[0];
    return any ? { $or: parts } : { $and: parts };
  }

  async mongo(fn) {
    try {
      return await fn();
    } catch (e) {
      throw new Error(`${this.name}: ${e.message}`, { cause: e });
    }
  }

  /** What Test connection shows: the server's version and its tables. */
  async describe() {
    await this.connect();
    const first = async (sql) => Object.values((await this.run(sql)).rows[0] || {})[0];
    const names = async (sql) => (await this.run(sql)).rows.map((r) => Object.values(r)[0]);
    switch (this.kind) {
      case "mariadb":
        return { version: String(await first("SELECT VERSION()")), tables: await names("SHOW TABLES") };
      case "postgres":
        return {
          version: String(await first("SHOW server_version")),
          tables: await names("SELECT table_name FROM information_schema.tables WHERE table_schema = current_schema() ORDER BY table_name"),
        };
      case "sqlite":
        return { version: String(await first("SELECT sqlite_version()")), tables: await names("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name"), file: this.file };
      case "mongodb": {
        const info = await this.mongo(() => this.db.admin().command({ buildInfo: 1 }));
        const list = await this.mongo(() => this.db.listCollections({}, { nameOnly: true }).toArray());
        return { version: info.version, tables: list.map((c) => c.name).sort() };
      }
    }
  }

  async close() {
    const client = this.client;
    this.client = null;
    this.connecting = null;
    if (!client) return;
    if (this.kind === "sqlite") client.close();
    else if (this.kind === "mongodb") await client.close();
    else await client.end();
  }

  // Stored values (Set/Get Stored Value) in the table xc_store: one row per value, as JSON.

  async prepareStore() {
    if (!this.sql) {
      await this.connect();
      return;
    }
    const text = this.kind === "mariadb" ? "VARCHAR(191)" : "TEXT";
    const value = this.kind === "mariadb" ? "LONGTEXT" : "TEXT";
    await this.run(`CREATE TABLE IF NOT EXISTS ${STORE_TABLE} (bucket ${text} NOT NULL, name ${text} NOT NULL, value ${value}, PRIMARY KEY (bucket, name))`);
  }

  async loadStore() {
    if (!this.sql) return this.mongo(async () => this.db.collection(STORE_TABLE).find({}).toArray());
    return (await this.run(`SELECT bucket, name, value FROM ${STORE_TABLE}`)).rows;
  }

  async writeStore(bucket, name, value) {
    const id = `${bucket}\u0000${name}`;
    if (!this.sql) {
      const c = this.db.collection(STORE_TABLE);
      if (value === undefined) await this.mongo(() => c.deleteOne({ _id: id }));
      else await this.mongo(() => c.updateOne({ _id: id }, { $set: { bucket, name, value: JSON.stringify(value) } }, { upsert: true }));
      return;
    }
    if (value === undefined) {
      await this.run(`DELETE FROM ${STORE_TABLE} WHERE bucket = ${this.ph(1)} AND name = ${this.ph(2)}`, [bucket, name]);
      return;
    }
    const upsert = this.kind === "mariadb" ? "ON DUPLICATE KEY UPDATE value = VALUES(value)" : "ON CONFLICT (bucket, name) DO UPDATE SET value = excluded.value";
    await this.run(`INSERT INTO ${STORE_TABLE} (bucket, name, value) VALUES (${this.ph(1)}, ${this.ph(2)}, ${this.ph(3)}) ${upsert}`, [bucket, name, JSON.stringify(value)]);
  }
}

/** A Discord (or Fluxer) object such as a user or channel: kept as its id. */
function isEntity(v) {
  return v && typeof v === "object" && !Array.isArray(v) && typeof v.id === "string" && Object.getPrototypeOf(v) !== Object.prototype && Object.getPrototypeOf(v) !== null;
}

/** A row as a plain object (drivers give their own classes). */
function plain(row) {
  return { ...row };
}

function mongoName(name) {
  const n = String(name ?? "").trim();
  if (!n || n.startsWith("$") || n.includes("\0")) throw new Error(`"${name}" can't be a collection or field name.`);
  return n;
}

/** A MongoDB document with its _id as text. */
function mongoPlain(doc) {
  const out = { ...doc };
  if (out._id && typeof out._id === "object" && typeof out._id.toHexString === "function") out._id = out._id.toHexString();
  return out;
}

/** Stored values (ctx.store) kept in a database instead of store.json. */
class DbStore extends Store {
  /** Opens the store in database `which`. The first time, store.json's values are copied in. */
  static async open(rt, which) {
    const store = Object.create(DbStore.prototype);
    Object.assign(store, { rt, data: {}, note: null, timer: null, dirty: new Map(), writing: null, readOnly: false });
    store.db = rt.databases.get(which);
    await store.db.prepareStore();
    const rows = await store.db.loadStore();
    for (const r of rows) {
      try {
        getOrObj(store.data, r.bucket)[r.name] = JSON.parse(r.value);
      } catch {
        rt.log.warn(`Stored value ${r.name} of ${r.bucket} in ${store.db.name} isn't JSON; it's left out.`);
      }
    }
    if (!rows.length) {
      const file = path.join(rt.opts.dataDir, "store.json");
      let old = null;
      try {
        old = JSON.parse(fs.readFileSync(file, "utf8"));
      } catch {}
      if (old && typeof old === "object" && !Array.isArray(old)) {
        let count = 0;
        for (const [bucket, values] of Object.entries(old)) {
          if (!values || typeof values !== "object") continue;
          for (const [name, value] of Object.entries(values)) {
            getOrObj(store.data, bucket)[name] = value;
            store.dirty.set(`${bucket}\u0000${name}`, [bucket, name]);
            count++;
          }
        }
        if (count) {
          await store.flushAll();
          store.note = `Stored values are kept in ${store.db.name} now: copied ${count} from store.json, which stays as it was.`;
        }
      }
    }
    openStores.add(store);
    return store;
  }

  changed(bucket, name) {
    this.dirty.set(`${bucket}\u0000${name}`, [bucket, name]);
    this.save();
  }

  save() {
    if (this.timer) return;
    this.timer = setTimeout(() => {
      this.timer = null;
      this.flushAll().catch(() => {});
    }, 250);
  }

  /** Writes the changed values; ones that fail stay waiting and are tried again. */
  async flushAll() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    // One write at a time, so a value never overtakes a newer one.
    while (this.writing) await this.writing;
    if (!this.dirty.size) return;
    const batch = [...this.dirty.values()];
    this.dirty.clear();
    this.writing = (async () => {
      for (const [bucket, name] of batch) {
        const values = this.data[bucket];
        try {
          await this.db.writeStore(bucket, name, values && name in values ? values[name] : undefined);
        } catch (e) {
          if (!this.dirty.has(`${bucket}\u0000${name}`)) this.dirty.set(`${bucket}\u0000${name}`, [bucket, name]);
          this.rt.log.error(`Could not save stored data: ${e.message}`);
          this.save();
          break;
        }
      }
    })();
    try {
      await this.writing;
    } finally {
      this.writing = null;
    }
  }

  flush() {
    this.flushAll().catch(() => {});
  }

  flushPending() {
    // Writing to a database takes time; a bot that ends waits for it in shutdown().
  }
}

/** Ends the bot after writing what's waiting and closing the databases (a few seconds at most). */
let shuttingDown = false;
async function shutdown(code = 0) {
  if (shuttingDown) return;
  shuttingDown = true;
  const wait = (ms) => new Promise((r) => setTimeout(r, ms).unref?.());
  await Promise.race([Promise.allSettled([...openStores].map((s) => s.flushAll())), wait(5000)]);
  await Promise.race([Promise.allSettled([...runtimes].map((rt) => rt.databases.closeAll())), wait(2000)]);
  process.exit(code);
}

/**
 * Bot › Settings › Databases › Test: connects with the connection in
 * XC_DB_TEST (JSON; its password and URL as XC_SECRET_DB_<ID>_…) and prints
 * one line of JSON: { ok, version, tables } or { ok: false, error }.
 */
async function testDatabase(depsRoot, dataDir) {
  const spec = JSON.parse(process.env.XC_DB_TEST || "{}");
  const rt = {
    opts: { dataDir },
    require: Module.createRequire(path.join(depsRoot, "package.json")),
    secret: (name) => process.env[`XC_SECRET_${String(name).toUpperCase()}`],
  };
  let db;
  try {
    db = new Database({ id: "test", ...spec }, rt);
    const info = await db.describe();
    console.log(JSON.stringify({ ok: true, ...info }));
  } catch (e) {
    console.log(JSON.stringify({ ok: false, error: e && e.message ? e.message : String(e) }));
  } finally {
    if (db) await Promise.race([db.close().catch(() => {}), new Promise((r) => setTimeout(r, 2000))]);
    process.exit(0);
  }
}

/**
 * XC Circuit's Stored Data, reading and changing a database: one request
 * (XC_DB_REQUEST, with the connection as `spec`) and one answer, the last
 * line printed: { ok: true, ... } or { ok: false, error }.
 */
async function browseDatabase(depsRoot, dataDir) {
  const req = JSON.parse(process.env.XC_DB_REQUEST || "{}");
  const rt = {
    opts: { dataDir },
    require: Module.createRequire(path.join(depsRoot, "package.json")),
    secret: (name) => process.env[`XC_SECRET_${String(name).toUpperCase()}`],
  };
  let db;
  try {
    db = new Database({ id: "browse", ...req.spec }, rt);
    const answer = await browse(db, req);
    console.log(JSON.stringify({ ok: true, ...answer }, browsable));
  } catch (e) {
    console.log(JSON.stringify({ ok: false, error: e && e.message ? e.message : String(e) }));
  } finally {
    if (db) await Promise.race([db.close().catch(() => {}), new Promise((r) => setTimeout(r, 2000))]);
    process.exit(0);
  }
}

/**
 * XC Circuit's Stored Data, kept connected: one database (XC_DB_SPEC), a
 * request per line on stdin ({ id, request }), its answer as a line on
 * stdout ({ id, ok, ... }). Requests are answered one at a time. It ends
 * when its input closes, or after ten minutes without a request.
 */
function serveDatabase(depsRoot, dataDir) {
  const spec = JSON.parse(process.env.XC_DB_SPEC || "{}");
  const rt = {
    opts: { dataDir },
    require: Module.createRequire(path.join(depsRoot, "package.json")),
    secret: (name) => process.env[`XC_SECRET_${String(name).toUpperCase()}`],
  };
  let db;
  try {
    db = new Database({ id: "browse", ...spec }, rt);
  } catch (e) {
    db = { close: async () => {}, failed: e };
  }
  const IDLE_MS = 10 * 60 * 1000;
  let idle = null;
  let quitting = false;
  const quit = async () => {
    if (quitting) return;
    quitting = true;
    await Promise.race([db.close().catch(() => {}), new Promise((r) => setTimeout(r, 2000))]);
    process.exit(0);
  };
  const bump = () => {
    clearTimeout(idle);
    idle = setTimeout(quit, IDLE_MS);
  };
  bump();
  let queue = Promise.resolve();
  const lines = require("readline").createInterface({ input: process.stdin });
  lines.on("line", (line) => {
    queue = queue.then(async () => {
      bump();
      let msg;
      try {
        msg = JSON.parse(line);
      } catch {
        return;
      }
      let out;
      try {
        if (db.failed) throw db.failed;
        out = { id: msg.id, ok: true, ...(await browse(db, msg.request || {})) };
      } catch (e) {
        out = { id: msg.id, ok: false, error: e && e.message ? e.message : String(e) };
      }
      process.stdout.write(JSON.stringify(out, browsable) + "\n");
    });
  });
  lines.on("close", () => queue.then(quit));
}

/** Values as Stored Data shows them: big numbers as text, binary data as its size. */
function browsable(key, value) {
  if (typeof value === "bigint") return value.toString();
  if (value && value.type === "Buffer" && Array.isArray(value.data)) return `(binary, ${value.data.length} bytes)`;
  if (value instanceof Uint8Array) return `(binary, ${value.length} bytes)`;
  return value;
}

/** What a row is known by: its key columns' values as conditions. Never none, so one row changes, not all. */
function keyConditions(key) {
  const conds = Object.entries(key || {}).map(([column, value]) => ({ column, op: "=", value }));
  if (!conds.length) throw new Error("This table has no primary key, so a row can't be told apart from the others: change it with your database's own tools.");
  return conds;
}

async function browse(db, req) {
  switch (req.op) {
    case "store":
      await db.prepareStore();
      return { values: (await db.loadStore()).map((r) => ({ bucket: r.bucket, name: r.name, value: r.value })) };
    case "store-set":
      await db.prepareStore();
      await db.writeStore(String(req.bucket), String(req.name), req.value ?? null);
      return {};
    case "store-delete":
      await db.prepareStore();
      for (const name of req.names || [req.name]) await db.writeStore(String(req.bucket), String(name), undefined);
      return {};
    case "tables":
      return { tables: (await db.describe()).tables };
    case "rows":
      return tableRows(db, String(req.table), req.offset, req.limit);
    case "row-insert":
      return { row: (await db.insert(String(req.table), req.values || {})).row };
    case "row-update":
      return { changed: await db.update(String(req.table), req.values || {}, keyConditions(req.key)) };
    case "row-delete":
      return { changed: await db.remove(String(req.table), keyConditions(req.key)) };
    default:
      throw new Error(`"${req.op}" isn't something Stored Data asks.`);
  }
}

/** A page of a table's rows, its columns, the columns that tell rows apart (its primary key), and how many rows it has. */
async function tableRows(db, table, offset, limit) {
  const n = Math.min(Math.max(Math.floor(Number(limit) || 50), 1), 500);
  const skip = Math.max(Math.floor(Number(offset) || 0), 0);
  if (!db.sql) {
    const rows = await db.select(table, [], { limit: n, offset: skip });
    const total = await db.mongo(() => db.db.collection(mongoName(table)).countDocuments());
    return { table, offset: skip, columns: [...new Set(["_id", ...rows.flatMap((r) => Object.keys(r))])], key: ["_id"], rows, total };
  }
  const t = db.ident(table);
  let columns = [];
  let key = [];
  switch (db.kind) {
    case "sqlite": {
      const info = (await db.run(`PRAGMA table_info(${t})`)).rows;
      columns = info.map((c) => c.name);
      key = info.filter((c) => Number(c.pk) > 0).sort((a, b) => a.pk - b.pk).map((c) => c.name);
      break;
    }
    case "mariadb": {
      const info = (await db.run(`SHOW COLUMNS FROM ${t}`)).rows;
      columns = info.map((c) => c.Field);
      key = info.filter((c) => c.Key === "PRI").map((c) => c.Field);
      break;
    }
    case "postgres": {
      const [schema, name] = table.includes(".") ? table.split(".") : [null, table];
      const params = schema ? [name, schema] : [name];
      columns = (await db.run(`SELECT column_name FROM information_schema.columns WHERE table_name = $1 AND table_schema = ${schema ? "$2" : "current_schema()"} ORDER BY ordinal_position`, params)).rows.map((r) => r.column_name);
      key = (await db.run("SELECT a.attname FROM pg_index i JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey) WHERE i.indrelid = $1::regclass AND i.indisprimary", [t])).rows.map((r) => r.attname);
      break;
    }
  }
  if (!columns.length) throw new Error(`${db.name} has no table called ${table}.`);
  let rows;
  // A SQLite table without a primary key still has a rowid to tell rows apart.
  if (db.kind === "sqlite" && !key.length) {
    rows = (await db.run(`SELECT rowid AS rowid, * FROM ${t}${db.limit(n, skip)}`)).rows;
    key = ["rowid"];
    columns = ["rowid", ...columns.filter((c) => c !== "rowid")];
  } else {
    rows = await db.select(table, [], { limit: n, offset: skip, order: key[0] });
  }
  const total = Number(Object.values((await db.run(`SELECT COUNT(*) AS n FROM ${t}`)).rows[0] || {})[0] || 0);
  return { table, offset: skip, columns, key, rows, total };
}

// ─── One-time timers (Schedule, On Scheduled) ────────────────────────────────
// Kept in the stored data (bucket ns:xc.timers), so they survive restarts and
// live in a database when stored values do. Each runs once, then is gone.

/** A timer that is this late counts as missed (the bot was off when it was due). */
const MISSED_AFTER_MS = 60 * 1000;
/** setTimeout can't wait longer than this; longer waits are split. */
const MAX_WAIT_MS = 2 ** 31 - 1;

/**
 * A value as it can be kept: JSON, with live objects (a Discord user, a
 * message) as their own data (toJSON, or their plain fields), dates as ISO
 * text, and nothing circular or deeper than a few levels.
 */
function snapshot(value, depth = 0, seen = new WeakSet()) {
  if (value === null || value === undefined) return value ?? null;
  if (typeof value === "bigint") return value.toString();
  if (typeof value !== "object") return typeof value === "function" ? undefined : value;
  if (value instanceof Date) return value.toISOString();
  if (seen.has(value) || depth > 5) return typeof value.id === "string" ? { id: value.id } : null;
  seen.add(value);
  if (typeof value.toJSON === "function" && !(value instanceof Map)) {
    try {
      return snapshot(value.toJSON(), depth + 1, seen);
    } catch {}
  }
  if (Array.isArray(value)) return value.slice(0, 1000).map((v) => snapshot(v, depth + 1, seen) ?? null);
  if (value instanceof Map) return Object.fromEntries([...value].slice(0, 1000).map(([k, v]) => [String(k), snapshot(v, depth + 1, seen)]));
  const out = {};
  for (const [k, v] of Object.entries(value)) {
    if (k.startsWith("_") || k === "client") continue;
    const s = snapshot(v, depth + 1, seen);
    if (s !== undefined) out[k] = s;
  }
  return out;
}

class Timers {
  constructor(rt) {
    this.rt = rt;
    this.listeners = new Map(); // name -> [{ fn, catchUp }]
    this.armed = new Map(); // id -> timeout
    this.started = false;
  }

  get scope() {
    return this.rt.store.scope("ns", "xc.timers");
  }

  /** Arms every saved timer: from now on, scheduling arms at once. */
  async start() {
    this.started = true;
    for (const t of await this.list()) this.arm(t);
  }

  arm(t) {
    clearTimeout(this.armed.get(t.id));
    const wait = Math.max(0, t.due - Date.now());
    const handle = setTimeout(() => (wait > MAX_WAIT_MS ? this.arm(t) : this.fire(t.id).catch((e) => this.rt.log.error(`Timer ${t.name}: ${e.message}`))), Math.min(wait, MAX_WAIT_MS));
    this.armed.set(t.id, handle);
  }

  /** Saves a timer and arms it. With a `key`, a timer with the same key is replaced. */
  async schedule({ name, due, data, key }) {
    name = String(name ?? "").trim();
    if (!name) throw new Error("A timer needs a name, which On Scheduled listens for");
    const at = due instanceof Date ? due.getTime() : Number(due);
    if (!Number.isFinite(at)) throw new Error("A timer needs a time to run at");
    let replaced = 0;
    if (key) replaced = await this.cancel({ key: String(key) });
    const t = { id: UTIL.id(12), name, due: Math.round(at), data: snapshot(data), created: Date.now() };
    if (key) t.key = String(key);
    await this.scope.set(t.id, t);
    if (this.started) this.arm(t);
    return { ...t, replaced: replaced > 0 };
  }

  /** Runs a due timer once: it's removed first, so it never runs twice. */
  async fire(id) {
    this.armed.delete(id);
    const t = await this.scope.get(id);
    if (!t) return;
    await this.scope.delete(id);
    const late = Math.max(0, Date.now() - t.due);
    const all = this.listeners.get(t.name) || [];
    if (!all.length) {
      this.rt.log.warn(`The timer "${t.name}" was due, but no On Scheduled block listens for it.`);
      return;
    }
    for (const l of all) {
      if (late > MISSED_AFTER_MS && !l.catchUp) continue;
      l.fn({ id: t.id, name: t.name, key: t.key ?? null, data: t.data, due: new Date(t.due), late });
    }
  }

  /** Removes timers by id, key or name (all of these that are given must match); gives how many. */
  async cancel({ id, key, name } = {}) {
    if (id == null && key == null && name == null) throw new Error("Say which timers: an id, a key or a name");
    let count = 0;
    for (const t of await this.list()) {
      if ((id != null && t.id !== String(id)) || (key != null && t.key !== String(key)) || (name != null && t.name !== String(name))) continue;
      clearTimeout(this.armed.get(t.id));
      this.armed.delete(t.id);
      await this.scope.delete(t.id);
      count++;
    }
    return count;
  }

  /** The waiting timers (of one name, if given), soonest first. */
  async list(name) {
    const s = this.scope;
    const out = [];
    for (const k of await s.keys()) {
      const t = await s.get(k);
      if (t && typeof t === "object" && t.id && (name == null || name === "" || t.name === name)) out.push(t);
    }
    return out.sort((a, b) => a.due - b.due);
  }

  on(name, fn, { catchUp = true } = {}) {
    getOr(this.listeners, String(name ?? "").trim(), () => []).push({ fn, catchUp });
  }

  api() {
    return {
      schedule: (o) => this.schedule(o),
      cancel: (o) => this.cancel(o),
      list: (name) => this.list(name),
      on: (name, fn, opts) => this.on(name, fn, opts),
    };
  }
}

// ─── Fetching what people point the bot at ──────────────────────────────────
// When the address comes from a ${…} or a wire, whoever types it decides where
// the bot connects: not this computer, its network or a cloud's metadata
// service. And nothing read from the web may be bigger than a limit.

/** Whether an IP address is somewhere only this computer or its network can reach. */
function isPrivateAddress(ip) {
  const net = require("net");
  let a = String(ip).toLowerCase();
  if (a.startsWith("[") && a.endsWith("]")) a = a.slice(1, -1);
  const mapped = a.match(/^::ffff:(\d+\.\d+\.\d+\.\d+)$/);
  if (mapped) a = mapped[1];
  if (net.isIPv4(a)) {
    const [p, q] = a.split(".").map(Number);
    return (
      p === 0 || p === 10 || p === 127 || (p === 100 && q >= 64 && q <= 127) || (p === 169 && q === 254) || (p === 172 && q >= 16 && q <= 31) || (p === 192 && q === 168) || (p === 198 && (q === 18 || q === 19)) || p >= 224
    );
  }
  if (net.isIPv6(a)) {
    return a === "::" || a === "::1" || /^f[cd]/.test(a) || /^fe[89ab]/.test(a) || a.startsWith("ff") || a.startsWith("64:ff9b:") || a.startsWith("2001:db8");
  }
  return true;
}

/** Throws unless every address `url`'s host has is public. */
async function checkPublic(url) {
  const u = new URL(url);
  if (!/^https?:$/.test(u.protocol)) throw new Error(`Only web addresses (https://…), not ${u.protocol}`);
  const host = u.hostname.replace(/^\[|\]$/g, "");
  const net = require("net");
  const addresses = net.isIP(host) ? [host] : (await require("dns").promises.lookup(host, { all: true, verbatim: true })).map((r) => r.address);
  if (!addresses.length || addresses.some(isPrivateAddress)) {
    throw new Error(`${u.host} is this computer or a private network; an address from what people type may only go to the public web. (Allow it with the block's Private addresses too.)`);
  }
}

/**
 * fetch(), guarded: with `publicOnly`, the address and every redirect must
 * be public; the body is read up to `maxBytes` (then it fails). Resolves to
 * { res, body } with the body as a Buffer.
 */
async function guardedFetch(url, init = {}, { publicOnly = false, maxBytes = 10 * 1024 * 1024, redirects = 5 } = {}) {
  let current = String(url);
  for (let hop = 0; ; hop++) {
    if (publicOnly) await checkPublic(current);
    const res = await fetch(current, { ...init, redirect: publicOnly ? "manual" : "follow" });
    const to = res.headers.get("location");
    if (publicOnly && res.status >= 300 && res.status < 400 && to) {
      if (hop >= redirects) throw new Error(`${new URL(current).host}: too many redirects`);
      current = new URL(to, current).href;
      // A redirect turns a POST into a GET, as browsers do.
      if (res.status === 303 || ((res.status === 301 || res.status === 302) && init.method && init.method !== "GET")) init = { ...init, method: "GET", body: undefined };
      continue;
    }
    const mb = Math.round((maxBytes / 1024 / 1024) * 10) / 10;
    if (Number(res.headers.get("content-length")) > maxBytes) throw new Error(`${new URL(current).host} sent more than ${mb} MB`);
    const tooBig = () => new Error(`${new URL(current).host} sent more than ${mb} MB`);
    // Read in pieces, stopping past the limit (a body without a stream, as
    // some stand-ins give: whole, then checked).
    if (!res.body || typeof res.body[Symbol.asyncIterator] !== "function") {
      const body = Buffer.from(await res.arrayBuffer());
      if (body.length > maxBytes) throw tooBig();
      return { res, body };
    }
    const chunks = [];
    let size = 0;
    for await (const chunk of res.body) {
      size += chunk.length;
      if (size > maxBytes) throw tooBig();
      chunks.push(chunk);
    }
    return { res, body: Buffer.concat(chunks) };
  }
}

const UTIL = {
  sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  /** fetch() with a size limit and, with publicOnly, only public addresses (redirects too): { res, body }. */
  fetch: guardedFetch,
  /** Throws unless a web address is public (not this computer or a private network). */
  checkPublic,
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
  compare,
};

/** A number, or a text that is one ("5", " 2.5 "); otherwise null. */
function asNumber(v) {
  if (typeof v === "number") return Number.isFinite(v) ? v : null;
  if (typeof v === "string" && v.trim() !== "" && Number.isFinite(Number(v))) return Number(v);
  return null;
}

/** What a value is compared as: a date's time, a Discord object's id. */
function comparable(v) {
  if (v instanceof Date) return v.getTime();
  if (v && typeof v === "object" && !Array.isArray(v) && typeof v.id === "string") return v.id;
  return v;
}

function textOf(v, ignoreCase) {
  const s = v == null ? "" : typeof v === "object" ? JSON.stringify(v) : String(v);
  return ignoreCase ? s.toLowerCase() : s;
}

/**
 * Whole numbers too long for JavaScript's numbers to hold exactly, such as
 * Discord ids (18-19 digits), as BigInts; null for anything else. Two
 * different ids would otherwise round to the same number.
 */
function asBigInt(v) {
  const s = typeof v === "bigint" ? v.toString() : typeof v === "string" ? v.trim() : typeof v === "number" && Number.isSafeInteger(v) ? String(v) : null;
  return s !== null && /^-?\d+$/.test(s) ? BigInt(s) : null;
}

/** Both as BigInts when either is a whole number longer than a number holds exactly. */
function bigPair(a, b) {
  const [x, y] = [asBigInt(a), asBigInt(b)];
  if (x === null || y === null) return null;
  const big = (n) => n > BigInt(Number.MAX_SAFE_INTEGER) || n < -BigInt(Number.MAX_SAFE_INTEGER);
  return big(x) || big(y) ? [x, y] : null;
}

function same(a, b, ignoreCase) {
  a = comparable(a);
  b = comparable(b);
  if (a == null || b == null) return (a ?? "") === (b ?? "");
  const big = bigPair(a, b);
  if (big) return big[0] === big[1];
  const [x, y] = [asNumber(a), asNumber(b)];
  if (x !== null && y !== null) return x === y;
  return textOf(a, ignoreCase) === textOf(b, ignoreCase);
}

/** Negative, zero or positive: numbers by value, dates by time, texts alphabetically. */
function order(a, b, ignoreCase) {
  a = comparable(a);
  b = comparable(b);
  const big = bigPair(a, b);
  if (big) return big[0] === big[1] ? 0 : big[0] > big[1] ? 1 : -1;
  const [x, y] = [asNumber(a), asNumber(b)];
  if (x !== null && y !== null) return x - y;
  return textOf(a, ignoreCase).localeCompare(textOf(b, ignoreCase));
}

function isEmpty(v) {
  if (v == null) return true;
  if (typeof v === "string") return v.trim() === "";
  if (Array.isArray(v)) return v.length === 0;
  if (typeof v.size === "number") return v.size === 0;
  if (typeof v === "object" && !(v instanceof Date)) return Object.keys(v).length === 0;
  return false;
}

/** `/pattern/flags` or a plain pattern. */
function pattern(p, ignoreCase) {
  const m = /^\/(.*)\/([a-z]*)$/s.exec(String(p ?? ""));
  const [source, flags] = m ? [m[1], m[2]] : [String(p ?? ""), ""];
  return new RegExp(source, ignoreCase && !flags.includes("i") ? flags + "i" : flags);
}

function contains(a, b, ignoreCase) {
  if (Array.isArray(a)) return a.some((x) => same(x, b, ignoreCase));
  if (a instanceof Map || (a && typeof a.has === "function" && typeof a.values === "function")) {
    return a.has(comparable(b)) || [...a.values()].some((x) => same(x, b, ignoreCase));
  }
  if (a && typeof a === "object") return Object.prototype.hasOwnProperty.call(a, String(b));
  return textOf(a, ignoreCase).includes(textOf(b, ignoreCase));
}

/**
 * The Compare and Check blocks: whether `a` passes `check` against `b` (and
 * `c`, the upper end for between). Numbers and numeric texts compare as
 * numbers, dates by time, Discord objects by id.
 */
function compare(a, check, b, { c, ignoreCase = false } = {}) {
  switch (check) {
    case "eq":
      return same(a, b, ignoreCase);
    case "ne":
      return !same(a, b, ignoreCase);
    case "gt":
      return order(a, b, ignoreCase) > 0;
    case "lt":
      return order(a, b, ignoreCase) < 0;
    case "gte":
      return order(a, b, ignoreCase) >= 0;
    case "lte":
      return order(a, b, ignoreCase) <= 0;
    case "between":
      return order(a, b, ignoreCase) >= 0 && order(a, c, ignoreCase) <= 0;
    case "contains":
      return contains(a, b, ignoreCase);
    case "not_contains":
      return !contains(a, b, ignoreCase);
    case "starts":
      return textOf(a, ignoreCase).startsWith(textOf(b, ignoreCase));
    case "ends":
      return textOf(a, ignoreCase).endsWith(textOf(b, ignoreCase));
    case "matches":
      return pattern(b, ignoreCase).test(textOf(a, false));
    case "one_of": {
      const options = Array.isArray(b) ? b : String(b ?? "").split(",").map((s) => s.trim());
      return options.some((o) => same(a, o, ignoreCase));
    }
    case "empty":
      return isEmpty(a);
    case "not_empty":
      return !isEmpty(a);
    case "number":
      return asNumber(a) !== null;
    case "true":
      return typeof a === "string" ? !["", "false", "0", "no", "off"].includes(a.trim().toLowerCase()) : Boolean(a);
    case "false":
      return !compare(a, "true", b);
    default:
      throw new Error(`Unknown check "${check}"`);
  }
}

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

/** Short texts for a block's outputs, for the editor's wire tooltips. */
function previews(outputs) {
  const out = {};
  for (const [k, v] of Object.entries(outputs || {})) out[k] = preview(v);
  return out;
}

/**
 * A value in a few words: texts quoted (and cut), lists with their first
 * items, Discord and Fluxer objects by class, name and id, anything else as
 * Node's inspect would show it.
 */
function preview(v, room = 300) {
  const cut = (s) => (s.length > room ? `${s.slice(0, room)}…` : s);
  if (v === undefined) return "nothing";
  if (v === null) return "null";
  if (typeof v === "string") return JSON.stringify(cut(v));
  if (typeof v === "function") return "a function";
  if (typeof v !== "object") return String(v);
  if (v instanceof Date) return Number.isNaN(v.getTime()) ? "Invalid date" : v.toISOString();
  if (Array.isArray(v)) {
    const first = v.slice(0, 3).map((x) => preview(x, 60));
    return `list of ${v.length}${first.length ? `: ${first.join(", ")}${v.length > 3 ? ", …" : ""}` : ""}`;
  }
  const kind = v.constructor && v.constructor.name;
  if (kind && kind !== "Object") {
    let label = "";
    try {
      label = v.tag ?? v.username ?? v.displayName ?? v.name ?? v.title ?? "";
    } catch {
      label = "";
    }
    const id = typeof v.id === "string" || typeof v.id === "number" ? ` (${v.id})` : "";
    return `${kind.replace(/^_/, "")}${label ? ` ${cut(String(label))}` : ""}${id}`;
  }
  return cut(inspect(v).replace(/\s+/g, " "));
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
  if (args.includes("--db-test")) {
    testDatabase(path.resolve(arg("--deps") || process.cwd()), path.resolve(arg("--data") || path.join(process.cwd(), "data")));
    return;
  }
  if (args.includes("--db-serve")) {
    serveDatabase(path.resolve(arg("--deps") || process.cwd()), path.resolve(arg("--data") || path.join(process.cwd(), "data")));
    return;
  }
  if (args.includes("--db-browse")) {
    browseDatabase(path.resolve(arg("--deps") || process.cwd()), path.resolve(arg("--data") || path.join(process.cwd(), "data")));
    return;
  }
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
for (const signal of ["SIGINT", "SIGTERM"]) process.on(signal, () => shutdown(0));

/** XC Circuit stops a running bot by writing "stop" to it (or by closing its input). */
function listenForStop() {
  process.stdin.setEncoding("utf8");
  let buffer = "";
  process.stdin.on("data", (chunk) => {
    buffer += chunk;
    const lines = buffer.split(/\r?\n/);
    buffer = lines.pop();
    for (const line of lines.map((l) => l.trim())) {
      if (line === "stop") shutdown(0);
      // "answer <id> <value>": XC Circuit's reply to an ask event.
      const m = /^answer (\S+) (.*)$/.exec(line);
      if (m && pendingAnswers.has(m[1])) {
        const resolve = pendingAnswers.get(m[1]);
        pendingAnswers.delete(m[1]);
        resolve(m[2]);
      }
    }
  });
  process.stdin.on("end", () => shutdown(0));
  process.stdin.on("error", () => {});
}

if (typeof __XC_BUNDLE__ !== "undefined") {
  loadDotEnv(path.join(__dirname, ".env"));
  start(__XC_BUNDLE__, { debug: process.argv.includes("--debug"), depsRoot: __dirname, dataDir: path.join(__dirname, "data") });
} else if (require.main === module) {
  cli();
}

module.exports = {
  isPrivateAddress, guardedFetch, start, ungroupBlock, fillTemplate, templateParts, snapshot, Timers, coerce, compare, preview, Store, DbStore, Database, Databases, backupStore, listStoreBackups, prepareStoreFile, BACKUPS_KEPT };
