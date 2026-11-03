// Checks how ${…} in texts are filled: each is a JavaScript expression.
// Usage: node runtime/test/template.js
"use strict";

const assert = require("assert");
const { fillTemplate, coerce, compare } = require("../circuit-runtime.js");

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

// Compare and Check: numbers typed as text are numbers, Discord objects compare by id.
const user = { id: "42", username: "erik" };
assert.ok(compare("10", "gt", 9), "10 > 9 as numbers, not as texts");
assert.ok(compare(10, "eq", "10.0"));
assert.ok(compare("b", "gt", "a") && !compare("Apple", "eq", "apple"));
assert.ok(compare("Apple", "eq", "apple", { ignoreCase: true }));
assert.ok(compare(user, "eq", "42") && compare(user, "eq", { id: "42" }));
assert.ok(compare(5, "between", 1, { c: "5" }) && !compare(6, "between", 1, { c: 5 }));
assert.ok(compare(new Date(2026, 0, 2), "gt", new Date(2026, 0, 1)));
assert.ok(compare("Hello world", "contains", "WORLD", { ignoreCase: true }) && compare("Hello", "not_contains", "x"));
assert.ok(compare(["1", "2"], "contains", 2), "lists: an item equal to it");
assert.ok(compare(new Map([["42", user]]), "contains", user), "collections: by key");
assert.ok(compare({ xp: 1 }, "contains", "xp"), "objects: a key");
assert.ok(compare("hello.png", "ends", ".PNG", { ignoreCase: true }) && compare("!ping", "starts", "!"));
assert.ok(compare("12345", "matches", "^\\d+$") && compare("HeLLo", "matches", "/hello/i") && compare("HeLLo", "matches", "hello", { ignoreCase: true }));
assert.ok(compare("red", "one_of", "red, green") && compare(3, "one_of", [1, "3"]) && !compare("blue", "one_of", "red, green"));
for (const e of [null, undefined, "", "  ", [], {}, new Map()]) assert.ok(compare(e, "empty"), `empty: ${JSON.stringify(e)}`);
assert.ok(compare(0, "not_empty") && compare(false, "not_empty"));
assert.ok(compare(" 2.5 ", "number") && !compare("2a", "number") && !compare(NaN, "number"));
assert.ok(compare("yes", "true") && compare("false", "false") && compare(0, "false"));
assert.ok(compare(null, "eq", "") && !compare(null, "eq", 0));
assert.throws(() => compare(1, "nope", 1), /Unknown check/);

console.log("template ok");
