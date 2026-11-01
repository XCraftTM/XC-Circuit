//! The editor-side QuickJS sandbox. It reads block files and runs the
//! editor-only hooks (`define()`). Bot code never runs here.

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
const __require = () => __inert();

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
    return JSON.stringify({ ok: out });
  } catch (e) {
    return __error(e);
  }
};

globalThis.__define = function (key, values, connected) {
  try {
    const block = __blocks[key];
    const result = block.define({ values: JSON.parse(values), connected: JSON.parse(connected), project: {} });
    return JSON.stringify({ ok: result || {} });
  } catch (e) {
    return __error(e);
  }
};

globalThis.circuit = {
  types: {
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
        let value = self.call("__load", (key.to_string(), source))?;
        Ok(serde_json::from_value(value)?)
    }

    /// Runs a block's `define()`.
    pub fn define(&self, key: &str, values: &Map<String, Value>, connected: &Map<String, Value>) -> Result<DefineResult> {
        let value = self.call(
            "__define",
            (key.to_string(), Value::Object(values.clone()).to_string(), Value::Object(connected.clone()).to_string()),
        )?;
        Ok(serde_json::from_value(value)?)
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
fn esm_to_commonjs(source: &str) -> String {
    source.replacen("export default", "module.exports =", 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use circuit_core::{BlockKind, BlockShape};
    use serde_json::json;

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

        let connected = json!({ "list": "list<discord.role>" });
        let result = engine.define("for_each", &Map::new(), connected.as_object().unwrap()).unwrap();
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
        assert!(engine.define("loop", &Map::new(), &Map::new()).is_err());
    }
}
