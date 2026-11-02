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

// {#n1.result} is block n1's output in this run, even when another block has
// an output of the same name; unknown blocks and outputs stay as they are.
assert.strictEqual(fillTemplate("{#n1.result} / {#n2.result} / {#n1.other}", run, {}), "42 / {#n2.result} / {#n1.other}");
const avatar = { id: "7", displayAvatarURL() { return `https://cdn/${this.id}.png`; }, delete() { throw new Error("must not run"); } };
const withUser = { ...run, outputs: new Map([["n3", { member: avatar }]]) };
assert.strictEqual(fillTemplate("{#n3.member.displayAvatarURL}", withUser, {}), "https://cdn/7.png", "methods that read are called");
assert.strictEqual(fillTemplate("[{#n3.member.delete}]", withUser, {}), "[]", "other methods never run");

console.log("template ok");
