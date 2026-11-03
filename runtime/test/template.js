// Checks how ${…} in texts are filled: each is a JavaScript expression.
// Usage: node runtime/test/template.js
"use strict";

const assert = require("assert");
const { fillTemplate, coerce } = require("../circuit-runtime.js");

const run = { event: { user: "from the event", guild: { name: "XC" } }, vars: new Map([["count", 3]]), outputs: new Map([["n1", { result: 42 }]]) };

// The event, run variables, the block's own fields and earlier outputs.
assert.strictEqual(fillTemplate("${user} in ${guild.name}", run, {}), "from the event in XC");
assert.strictEqual(fillTemplate("${count} times", run, {}), "3 times");
assert.strictEqual(fillTemplate("${mode}", run, { mode: "reply" }), "reply");
assert.strictEqual(fillTemplate("answer ${result}", run, {}), "answer 42");

// A value wired into the text's input wins over the event's.
assert.strictEqual(fillTemplate("Hi ${user}!", run, {}, { user: "wired" }), "Hi wired!");
assert.strictEqual(fillTemplate("${who.name}", run, {}, { who: { name: "Ada" } }), "Ada");

// Any JavaScript: arithmetic, methods, ?? and ?., JavaScript's own names,
// strings with braces, nested template strings.
assert.strictEqual(fillTemplate("${count * 2 + 1}", run, {}), "7");
assert.strictEqual(fillTemplate("${guild.name.toLowerCase()}", run, {}), "xc");
assert.strictEqual(fillTemplate("${run?.missing ?? 'none'}", run, { run: null }), "none");
assert.strictEqual(fillTemplate("${Math.max(count, 10)}", run, {}), "10");
assert.strictEqual(fillTemplate("${'{' + count + '}'}", run, {}), "{3}");
assert.strictEqual(fillTemplate("${`${count} times`}!", run, {}), "3 times!");
assert.strictEqual(fillTemplate("${ {a: count}.a }", run, {}), "3");

// #n1 is block n1's outputs in this run, even when another block has an
// output of the same name; inside strings # is just a #.
assert.strictEqual(fillTemplate("${#n1.result} ${'#n1'}", run, {}), "42 #n1");
const avatar = { id: "7", displayAvatarURL() { return `https://cdn/${this.id}.png`; } };
const withUser = { ...run, outputs: new Map([["3", { member: avatar }]]) };
assert.strictEqual(fillTemplate("${#3.member.displayAvatarURL()}", withUser, {}), "https://cdn/7.png");

// Text without ${ is left alone, old-style braces too; empty values become "".
assert.strictEqual(fillTemplate("{user} \\d{3} $5", run, {}), "{user} \\d{3} $5");
assert.strictEqual(fillTemplate("[${gone}]", run, {}, { gone: null }), "[]");
assert.strictEqual(fillTemplate("an open ${count", run, {}), "an open ${count");

// Mistakes name the expression.
assert.throws(() => fillTemplate("${nobody}", run, {}), /\$\{nobody\}: nobody is not defined/);
assert.throws(() => fillTemplate("${count +}", run, {}), /\$\{count \+\}/);

// One value into a list field is a list of one; lists and collections stay as they are.
const embed = { title: "Hi" };
assert.deepStrictEqual(coerce(embed, "list<discord.embed>"), [embed]);
assert.deepStrictEqual(coerce("a", "list<text>?"), ["a"]);
assert.deepStrictEqual(coerce([embed], "list<discord.embed>"), [embed]);
const roles = new Map([["1", { name: "Mod" }]]);
assert.strictEqual(coerce(roles, "list<discord.role>"), roles);
assert.strictEqual(coerce(undefined, "list<any>"), undefined);

console.log("template ok");
