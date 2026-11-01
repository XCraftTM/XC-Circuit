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
    this.startHandlers = [];
    this.store = new Store(opts.dataDir, this);
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
    const req = (name) => this.require(name);
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
    this.log.info(`Runtime ready · Node ${process.version}`);
    for (const [id, b] of Object.entries(this.bundle.blocks)) {
      try {
        this.blocks.set(id, this.loadModule(b.source.replace("export default", "module.exports ="), b.file));
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

  ctxFor(run, node) {
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
      abortSignal: run ? run.abortSignal : undefined,
      next: async (flow, outputs) => {
        if (!run) throw new Error("ctx.next() can only be used while a block runs");
        if (outputs) run.setOutputs(node.id, outputs);
        await graph.follow(run, node.id, flow);
      },
      set: (output, value) => {
        if (run) run.setOutputs(node.id, { [output]: value });
      },
      bot: {
        get uptime() {
          return Date.now() - rt.startedAt;
        },
        stop: () => process.exit(0),
        restart: () => process.exit(75),
      },
      runtime: {
        onError: (h) => rt.errorHandlers.push(h),
        onSignal: (name, h) => getOr(rt.signalHandlers, name, () => []).push(h),
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
  uncaught(err, node) {
    const details = describeError(err, node);
    this.emit("node", { ws: node.graph.key, node: node.id, phase: "error", error: details.message });
    if (this.errorHandlers.length) {
      for (const h of this.errorHandlers) h({ message: details.message, details, block: details.block });
    } else {
      this.log.error(`${node.graph.name} › ${node.def.name || node.blockId} (#${node.id}): ${details.message}`);
    }
  }
}

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
    this.follow(run, node.id, "next").catch((e) => this.rt.uncaught(e, e.__xcNode || node));
  }

  async follow(run, nodeId, flow) {
    const targets = this.flowsOut.get(`${nodeId}.${flow}`) || [];
    await Promise.all(targets.map((t) => this.execute(run, t, run.zoneStack)));
  }

  /** Runs an action block, entering its Try Zone when needed. */
  async execute(run, nodeId, zoneStack) {
    const node = this.nodes.get(nodeId);
    if (!node) return;
    const zone = node.data.zone;
    if (zone && this.zones.has(zone) && !zoneStack.includes(zone)) {
      return this.runZone(run, zone, nodeId, zoneStack);
    }
    if (!zone && zoneStack.length) {
      // An Action wire may only leave a zone through its own outputs; treat it as leaving.
      return this.execute(run, nodeId, []);
    }
    return this.executeNode(run, node, zoneStack);
  }

  async executeNode(run, node, zoneStack) {
    const t0 = performance.now();
    this.rt.emit("node", { ws: this.key, node: node.id, phase: "enter", run: run.id });
    const prevStack = run.zoneStack;
    run.zoneStack = zoneStack;
    try {
      if (run.abortSignal && run.abortSignal.aborted) throw new Error("timed out");
      const fields = await node.resolve(run);
      const ctx = this.rt.ctxFor(run, node);
      const result = await node.def.run({ fields, ctx, node: { id: node.id, workspace: this.key } });
      if (result && typeof result === "object") run.setOutputs(node.id, result);
      this.rt.emit("node", { ws: this.key, node: node.id, phase: "exit", run: run.id, ms: +(performance.now() - t0).toFixed(1) });
      if (!node.def.flows) await this.follow(run, node.id, "next");
    } catch (e) {
      if (!e.__xcReported) {
        e.__xcReported = true;
        e.__xcNode = e.__xcNode || node;
        this.rt.emit("node", { ws: this.key, node: node.id, phase: "error", run: run.id, error: String(e.message || e) });
      }
      throw e;
    } finally {
      run.zoneStack = prevStack;
    }
  }

  async runZone(run, zoneId, entryId, zoneStack) {
    const zone = this.zones.get(zoneId);
    const attempts = 1 + Math.max(0, zone.retry || 0);
    let lastError;
    for (let attempt = 1; attempt <= attempts; attempt++) {
      const controller = new AbortController();
      const parentSignal = run.abortSignal;
      run.abortSignal = controller.signal;
      let timer;
      try {
        const work = this.execute(run, entryId, [...zoneStack, zoneId]);
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
        run.abortSignal = parentSignal;
        this.rt.emit("zone", { ws: this.key, zone: zoneId, phase: "done", run: run.id });
        return this.follow(run, zoneId, "done");
      } catch (e) {
        clearTimeout(timer);
        controller.abort();
        run.abortSignal = parentSignal;
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
    return this.follow(run, zoneId, "error");
  }

  /** Reads an output: from blocks that already ran in this run, or by computing a value block. */
  async read(run, nodeId, pin) {
    const done = run.outputs.get(nodeId);
    if (done && pin in done) return done[pin];
    const node = this.nodes.get(nodeId);
    if (!node) return undefined;
    if (node.def.kind !== "value") return done ? done[pin] : undefined;
    const fields = await node.resolve(run);
    const result = (await node.def.run({ fields, ctx: this.rt.ctxFor(run, node), node: { id: node.id, workspace: this.key } })) || {};
    return result[pin];
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
  async resolve(run) {
    const fields = this.inline();
    const wired = new Set();
    for (const [key, sources] of this.graph.inputs) {
      const [nodeId, pin] = splitPin(key);
      if (nodeId !== this.id) continue;
      const values = await Promise.all(sources.map((s) => this.graph.read(run, s.node, s.pin)));
      const def = (this.def.fields || {})[pin];
      fields[pin] = def && def.multi ? values.flat() : values[0];
      wired.add(pin);
    }
    for (const [k, f] of Object.entries(this.def.fields || {})) {
      if (!f) continue;
      if (f.template && !wired.has(k) && typeof fields[k] === "string") fields[k] = fillTemplate(fields[k], run, fields);
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
    this.zoneStack = [];
    this.abortSignal = undefined;
  }

  setOutputs(nodeId, outputs) {
    this.outputs.set(nodeId, Object.assign(this.outputs.get(nodeId) || {}, outputs));
  }
}

/** Fills {name} and {name.path} from the event, run variables and this block's other fields. */
function fillTemplate(text, run, fields) {
  return text.replace(/\{([\w$.]+)\}/g, (whole, expr) => {
    const [head, ...rest] = expr.split(".");
    let value;
    if (run && run.event && head in run.event) value = run.event[head];
    else if (run && run.vars.has(head)) value = run.vars.get(head);
    else if (head in fields) value = fields[head];
    else if (run && lastOutput(run, head) !== NOT_FOUND) value = lastOutput(run, head);
    else return whole;
    for (const k of rest) value = value == null ? undefined : value[k];
    return value == null ? "" : String(value);
  });
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

/** Saved data (ctx.store): one JSON file, written safely and in batches. */
class Store {
  constructor(dir, rt) {
    this.rt = rt;
    this.file = path.join(dir, "store.json");
    this.data = {};
    try {
      this.data = JSON.parse(fs.readFileSync(this.file, "utf8"));
    } catch {
      this.data = {};
    }
    this.timer = null;
  }

  save() {
    if (this.timer) return;
    this.timer = setTimeout(() => {
      this.timer = null;
      try {
        fs.mkdirSync(path.dirname(this.file), { recursive: true });
        const tmp = this.file + ".tmp";
        fs.writeFileSync(tmp, JSON.stringify(this.data, null, 2));
        fs.renameSync(tmp, this.file);
      } catch (e) {
        this.rt.log.error(`Could not save stored data: ${e.message}`);
      }
    }, 250);
  }

  scope(...parts) {
    const key = parts.map((p) => String(p?.id ?? p)).join(":");
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
    };
  }

  api() {
    return {
      global: () => this.scope("global"),
      server: (id) => this.scope("server", id),
      user: (id) => this.scope("user", id),
      member: (serverId, userId) => this.scope("member", serverId, userId),
      channel: (id) => this.scope("channel", id),
      ns: (name) => this.scope("ns", name),
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
  shuffle: (list) => [...list].sort(() => Math.random() - 0.5),
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
    const m = /^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)\s*$/.exec(line);
    if (!m || m[1] in process.env) continue;
    process.env[m[1]] = m[2].replace(/^(['"])(.*)\1$/, "$2");
  }
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
  start(bundle, {
    debug: args.includes("--debug"),
    depsRoot: arg("--deps") || process.cwd(),
    dataDir: arg("--data") || path.join(process.cwd(), "data"),
  });
}

process.on("unhandledRejection", (e) => {
  const text = `Unhandled error: ${e && e.stack ? e.stack : e}`;
  if (process.argv.includes("--debug")) process.stdout.write(PROTOCOL + JSON.stringify({ t: "log", level: "error", text }) + "\n");
  else console.error(text);
});

if (typeof __XC_BUNDLE__ !== "undefined") {
  loadDotEnv(path.join(__dirname, ".env"));
  start(__XC_BUNDLE__, { debug: process.argv.includes("--debug"), depsRoot: __dirname, dataDir: path.join(__dirname, "data") });
} else if (require.main === module) {
  cli();
}

module.exports = { start, fillTemplate, coerce };
