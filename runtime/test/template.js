// Checks how {placeholders} in text are filled.
// Usage: node runtime/test/template.js
"use strict";

const assert = require("assert");
const { fillTemplate } = require("../circuit-runtime.js");

const run = { event: { user: "from the event", guild: { name: "XC" } }, vars: new Map([["count", 3]]), outputs: new Map([["n1", { result: 42 }]]) };

// The event, run variables, the block's own fields and earlier outputs.
assert.strictEqual(fillTemplate("{user} in {guild.name}", run, {}), "from the event in XC");
assert.strictEqual(fillTemplate("{count} times", run, {}), "3 times");
assert.strictEqual(fillTemplate("{mode}", run, { mode: "reply" }), "reply");
assert.strictEqual(fillTemplate("answer {result}", run, {}), "answer 42");

// A value wired into the field's {user} input wins over the event's user.
assert.strictEqual(fillTemplate("Hi {user}!", run, {}, { user: "wired" }), "Hi wired!");
assert.strictEqual(fillTemplate("{who.name}", run, {}, { who: { name: "Ada" } }), "Ada");

// Unknown names and non-names stay as they are; empty values become "".
assert.strictEqual(fillTemplate("{nobody} \\d{3}", run, {}), "{nobody} \\d{3}");
assert.strictEqual(fillTemplate("[{gone}]", run, {}, { gone: null }), "[]");

console.log("template ok");
