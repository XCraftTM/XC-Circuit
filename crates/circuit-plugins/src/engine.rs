//! The editor-side QuickJS sandbox. It reads block files and runs the
//! editor-only hooks (`define()`). Bot code never runs here.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use circuit_core::{BlockDef, DefineResult};
use rquickjs::{Context, Function, Runtime};
use serde::Deserialize;
use serde_json::{Map, Value};

/// How long one call into a block file may run.
const BUDGET: Duration = Duration::from_millis(50);
const MEMORY_LIMIT: usize = 64 * 1024 * 1024;

/// Sets up `module`/`require` shims and the helpers blocks may use in `define()`.
const PRELUDE: &str = r#"
"use strict";
globalThis.__blocks = Object.create(null);

// Top-level `require()` calls must not break loading in the editor, so they return
// an inert stand-in. Real modules are only available in the bot.
const __inert = () => new Proxy(function () {}, {
  get: (_, key) => (key === Symbol.toPrimitive ? () => "" : __inert()),
  apply: () => __inert(),
  construct: () => __inert(),
});
const __sdk = { defineBlock: (block) => block };
const __require = (name) => (name === "@xc-circuit/sdk" ? __sdk : __inert());

function __error(e) {
  return JSON.stringify({ error: String(e && e.stack ? e.message + "\n" + e.stack : e) });
}

globalThis.__load = function (key, src) {
  try {
    const module = { exports: {} };
    new Function("module", "exports", "require", src)(module, module.exports, __require);
    let block = module.exports;
    if (block && block.default) block = block.default;
    if (!block || typeof block !== "object") throw new Error("the file does not export a block object");
    __blocks[key] = block;
    const out = {};
    for (const k of Object.keys(block)) {
      if (typeof block[k] === "function") out["has_" + k] = true;
      else out[k] = block[k];
    }
    // Async blocks wait for something before the flow goes on: their run()
    // awaits or chains a promise. Waiting for the rest of the flow
    // (ctx.next) doesn't count, nor does the async keyword on its own.
    if (typeof block.run === "function") {
      const own = Function.prototype.toString.call(block.run).replace(/\b(await|return)\s+ctx\.next\s*\(/g, "ctx.next(");
      out.async_run = /\bawait\b|\.then\s*\(|\bnew\s+Promise\b|\bPromise\.(all|allSettled|any|race)\s*\(/.test(own);
    }
    return JSON.stringify({ ok: out });
  } catch (e) {
    return __error(e);
  }
};

globalThis.__define = function (key, values, connected, project) {
  try {
    const block = __blocks[key];
    const result = block.define({ values: JSON.parse(values), connected: JSON.parse(connected), project: JSON.parse(project) });
    return JSON.stringify({ ok: result || {} });
  } catch (e) {
    return __error(e);
  }
};

globalThis.__migrate = function (key, values, from) {
  try {
    const result = __blocks[key].migrate(JSON.parse(values), from);
    return JSON.stringify({ ok: result || {} });
  } catch (e) {
    return __error(e);
  }
};

// The class catalogs (contributes.classes): what the values behind pin types have.
globalThis.__classes = { roots: {}, classes: {} };
globalThis.__setClasses = function (json) {
  try {
    __classes = JSON.parse(json);
    return JSON.stringify({ ok: true });
  } catch (e) {
    return __error(e);
  }
};

globalThis.circuit = {
  types: {
    // "discord.user" -> { username: "Text", id: "Text", … }: the properties a
    // pin type's values have, from the class catalogs; null when unknown.
    members(type) {
      const cls = __classes.roots[String(type || "").trim().replace(/\?$/, "")];
      const def = cls && __classes.classes[cls];
      if (!def) return null;
      const out = {};
      for (const m of def.members.filter((m) => m.kind !== "method").sort((a, b) => a.name.localeCompare(b.name))) out[m.name] = m.type || "any";
      return out;
    },
    // "object<{ name: text, xp: number }>" -> { name: "text", xp: "number" }
    shape(type) {
      const m = /^object<\{(.*)\}>$/.exec(String(type || "").trim());
      if (!m) return null;
      const out = {};
      for (const part of m[1].split(",")) {
        const [k, v] = part.split(":").map((s) => s && s.trim());
        if (k) out[k] = v || "any";
      }
      return out;
    },
    elementOf(type) {
      const m = /^list<(.*)>$/.exec(String(type || "").trim());
      return m ? m[1] : "any";
    },
  },
  object: {
    keysToChoices(shape) {
      const out = {};
      for (const k of Object.keys(shape || {})) out[k] = k;
      return out;
    },
  },
};
"#;

#[derive(Deserialize)]
struct Reply {
    ok: Option<Value>,
    error: Option<String>,
}

pub struct BlockEngine {
    runtime: Runtime,
    context: Context,
    deadline: Arc<Mutex<Option<Instant>>>,
}

impl BlockEngine {
    pub fn new() -> Result<Self> {
        let runtime = Runtime::new()?;
        runtime.set_memory_limit(MEMORY_LIMIT);
        let deadline: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let watch = deadline.clone();
        runtime.set_interrupt_handler(Some(Box::new(move || {
            watch.lock().ok().and_then(|d| *d).is_some_and(|d| Instant::now() > d)
        })));
        let context = Context::full(&runtime)?;
        context.with(|ctx| ctx.eval::<(), _>(PRELUDE))?;
        Ok(Self { runtime, context, deadline })
    }

    /// Loads a block file. `key` identifies it for later `define()` calls.
    pub fn load_block(&self, key: &str, source: &str) -> Result<BlockDef> {
        let source = esm_to_commonjs(source);
        let mut value = self.call("__load", (key.to_string(), source))?;
        let problems = circuit_core::block::ungroup(&mut value);
        let mut def: BlockDef = serde_json::from_value(value)?;
        def.load_problems = problems;
        Ok(def)
    }

    /// Runs a block's `define()`. `project` is what blocks see as `project`
    /// (currently `{ data }`, such as the slash commands).
    pub fn define(&self, key: &str, values: &Map<String, Value>, connected: &Map<String, Value>, project: &Value) -> Result<DefineResult> {
        let value = self.call(
            "__define",
            (
                key.to_string(),
                Value::Object(values.clone()).to_string(),
                Value::Object(connected.clone()).to_string(),
                project.to_string(),
            ),
        )?;
        Ok(serde_json::from_value(value)?)
    }

    /// Gives blocks' `define()` the class catalogs, for `circuit.types.members`:
    /// each pin type's class (`roots`) and the properties classes have.
    pub fn set_classes(&self, roots: &HashMap<String, String>, classes: &HashMap<String, crate::ClassDef>) -> Result<()> {
        let classes: serde_json::Map<String, Value> = classes
            .iter()
            .map(|(name, c)| {
                let members: Vec<Value> = c.members.iter().map(|m| serde_json::json!({ "name": m.name, "kind": m.kind, "type": m.ty })).collect();
                (name.clone(), serde_json::json!({ "members": members }))
            })
            .collect();
        self.call("__setClasses", (serde_json::json!({ "roots": roots, "classes": classes }).to_string(),))?;
        Ok(())
    }

    /// Runs a block's `migrate(values, fromVersion)` to upgrade saved values.
    pub fn migrate(&self, key: &str, values: &Map<String, Value>, from: &str) -> Result<Map<String, Value>> {
        let value = self.call("__migrate", (key.to_string(), Value::Object(values.clone()).to_string(), from.to_string()))?;
        match value {
            Value::Object(m) => Ok(m),
            _ => Ok(values.clone()),
        }
    }

    fn call<A>(&self, function: &str, args: A) -> Result<Value>
    where
        A: for<'js> rquickjs::function::IntoArgs<'js>,
    {
        self.set_deadline(Some(Instant::now() + BUDGET));
        let text: rquickjs::Result<String> = self.context.with(|ctx| {
            let f: Function = ctx.globals().get(function)?;
            f.call(args)
        });
        self.set_deadline(None);
        self.runtime.run_gc();
        let text = text.map_err(|e| match e {
            rquickjs::Error::Exception => anyhow!("took longer than {} ms or threw outside the sandbox", BUDGET.as_millis()),
            other => anyhow!(other),
        })?;
        let reply: Reply = serde_json::from_str(&text)?;
        match (reply.ok, reply.error) {
            (_, Some(err)) => Err(anyhow!(err)),
            (Some(ok), None) => Ok(ok),
            (None, None) => Ok(Value::Null),
        }
    }

    fn set_deadline(&self, deadline: Option<Instant>) {
        if let Ok(mut d) = self.deadline.lock() {
            *d = deadline;
        }
    }
}

/// Lets block files use `export default { … }` instead of `module.exports = { … }`.
/// Only a line starting with it counts, never the words inside a string or
/// a comment.
fn esm_to_commonjs(source: &str) -> String {
    let mut done = false;
    let lines: Vec<String> = source
        .split('\n')
        .map(|line| {
            let rest = line.trim_start();
            match rest.strip_prefix("export default").filter(|r| !done && r.starts_with(|c: char| c.is_whitespace() || c == '{' || c == '(')) {
                Some(r) => {
                    done = true;
                    format!("{}module.exports ={r}", &line[..line.len() - rest.len()])
                }
                None => line.to_string(),
            }
        })
        .collect();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use circuit_core::{BlockKind, BlockShape};
    use serde_json::json;

    #[test]
    fn grouped_block_files_load_like_flat_ones() {
        let engine = BlockEngine::new().unwrap();
        let src = r#"module.exports = {
            id: "x.grouped",
            kind: "action",
            info: { name: "Grouped", description: "D", author: "A", version: "2.1.0" },
            search: { category: "send", aliases: ["Reply"], tags: ["dm"] },
            look: { icon: "message", width: 300 },
            requires: { intents: ["Guilds"], dependencies: { "left-pad": "1" } },
            run() {},
        };"#;
        let mut def = engine.load_block("grouped", src).unwrap();
        assert!(def.finish().is_empty());
        assert_eq!((def.name.as_str(), def.version.as_str(), def.category.as_deref(), def.icon.as_deref()), ("Grouped", "2.1.0", Some("send"), Some("message")));
        assert_eq!((def.tags.clone(), def.intents.clone(), def.width), (vec!["dm".to_string()], vec!["Guilds".to_string()], Some(300.0)));
        assert_eq!(def.dependencies["left-pad"], "1");

        let wrong = r#"module.exports = { id: "x.w", kind: "action", name: "W", info: { name: "W", description: "D", author: "A", version: "1.0.0", colour: "red" }, run() {} };"#;
        let problems = engine.load_block("wrong", wrong).unwrap().finish();
        assert!(problems.iter().any(|p| p.contains("`info` has no `colour`")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("`name` is given twice")), "{problems:?}");
    }

    #[test]
    fn export_default_only_at_the_start_of_a_line() {
        let src = "// says \"export default\" here\nconst s = \"export default\";\nexport default { id: \"x\" };";
        assert_eq!(esm_to_commonjs(src), "// says \"export default\" here\nconst s = \"export default\";\nmodule.exports = { id: \"x\" };");
        assert_eq!(esm_to_commonjs("  export default{}"), "  module.exports ={}");
        assert_eq!(esm_to_commonjs("module.exports = {};"), "module.exports = {};");
    }

    const FOR_EACH: &str = r#"
        const lodash = require("lodash");
        module.exports = {
          id: "core.for_each", name: "For Each", description: "d", author: "a", version: "1.0.0",
          kind: "action", flows: ["each", "done"],
          fields: { list: { type: "list<any>", pin: "only" } },
          outputs: { item: { type: "any" } },
          define({ connected }) {
            return { outputs: { item: { type: circuit.types.elementOf(connected.list) } } };
          },
          async run() {},
        };
    "#;

    #[test]
    fn loads_block_and_runs_define() {
        let engine = BlockEngine::new().unwrap();
        let mut def = engine.load_block("for_each", FOR_EACH).unwrap();
        assert!(def.finish().is_empty());
        assert_eq!(def.kind, BlockKind::Action);
        assert!(def.has_define && def.has_run);
        // `async run() {}` alone isn't waiting for anything.
        assert_eq!(def.timing(), Some(false));
        let timing = |src: &str| engine.load_block("t", &format!("module.exports = {{ id: 'x.t', kind: 'action', {src} }}")).unwrap().timing();
        assert_eq!(timing("run() { return {} }"), Some(false));
        assert_eq!(timing("async run({ ctx }) { await ctx.next('a') }"), Some(false), "waiting for the rest of the flow isn't waiting");
        assert_eq!(timing("async run({ ctx }) { const m = await ctx.store.global().get('x'); return { m } }"), Some(true));
        assert_eq!(timing("run({ fields }) { return fields.member.roles.add('1').then(() => ({})) }"), Some(true));
        assert_eq!(timing("run() { return new Promise((r) => setTimeout(r, 5)) }"), Some(true));

        let connected = json!({ "list": "list<discord.role>" });
        let result = engine.define("for_each", &Map::new(), connected.as_object().unwrap(), &json!({})).unwrap();
        let shape = BlockShape::build(&def, &Map::new(), Some(&result));
        assert_eq!(shape.output("item").unwrap().type_name, "discord.role");
        assert_eq!(shape.flows, vec!["each", "done"]);
    }

    #[test]
    fn reports_errors_and_stops_endless_loops() {
        let engine = BlockEngine::new().unwrap();
        let err = engine.load_block("bad", "module.exports = 5").unwrap_err();
        assert!(err.to_string().contains("does not export a block"));
        engine
            .load_block("loop", "module.exports = { define() { while (true) {} } }")
            .unwrap();
        assert!(engine.define("loop", &Map::new(), &Map::new(), &json!({})).is_err());
    }
}
