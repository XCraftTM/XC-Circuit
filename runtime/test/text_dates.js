// Split Text, Date & Time, Read Date and Time Between: what they give for
// what people type.
// Usage: node runtime/test/text_dates.js
"use strict";

const path = require("path");
const assert = require("assert");
const { ungroupBlock } = require("../circuit-runtime.js");

const dir = path.join(__dirname, "..", "..", "plugins", "xc.core", "blocks");
const block = (rel) => ungroupBlock(require(path.join(dir, rel)));
const run = (rel, fields) => {
  const b = block(rel);
  const defaults = Object.fromEntries(Object.entries(b.fields).map(([k, f]) => [k, f.default ?? null]));
  const out = b.run({ fields: { ...defaults, ...fields }, ctx: {} });
  for (const key of Object.keys(b.outputs)) assert.ok(key in out, `${b.id}: output ${key}`);
  return out;
};

// ---- Split Text ----
let o = run("text/split_text.js", { text: "!kick  @ada   being rude" });
assert.deepStrictEqual([o.first, o.last, o.part, o.rest, o.count], ["!kick", "rude", "@ada", "@ada being rude", 4], "spaces, several as one");
o = run("text/split_text.js", { text: "red, green , blue,", separator: ",", number: -2 });
assert.deepStrictEqual([o.first, o.last, o.part, o.rest, o.parts], ["red", "blue", "green", "green,blue", ["red", "green", "blue"]], "trimmed, the empty end skipped, -2 from the end");
o = run("text/split_text.js", { text: "a||b", separator: "|", skipEmpty: false, number: 2 });
assert.deepStrictEqual([o.part, o.count], ["", 3], "empty parts kept when asked");
o = run("text/split_text.js", { text: "", number: 5 });
assert.deepStrictEqual([o.first, o.last, o.part, o.rest, o.count], ["", "", "", "", 0], "nothing to split");

// ---- Date & Time ----
const xmas = new Date(Date.UTC(2026, 11, 24, 17, 0, 0));
o = run("dates/now.js", { date: xmas, add: "1d 2h 30m", format: "iso" });
assert.strictEqual(o.text, "2026-12-25T19:30:00.000Z", "Add takes days, hours and minutes");
assert.strictEqual(run("dates/now.js", { date: xmas, add: "-1w", format: "iso" }).text, "2026-12-17T17:00:00.000Z");
assert.strictEqual(run("dates/now.js", { date: xmas, add: "90", format: "iso" }).text, "2026-12-24T18:30:00.000Z", "a plain number is minutes");
assert.strictEqual(run("dates/now.js", { date: xmas, addMinutes: 10, format: "iso" }).text, "2026-12-24T17:10:00.000Z", "older projects' minutes still work");
o = run("dates/now.js", { date: xmas, format: "custom", pattern: "dddd, DD.MM.YYYY [at] HH:mm", timeZone: "Europe/Berlin" });
assert.strictEqual(o.text, "Thursday, 24.12.2026 at 18:00", "a pattern in a time zone");
assert.strictEqual(run("dates/now.js", { date: xmas, format: "custom", pattern: "D MMMM YYYY, h:mm A", timeZone: "America/New_York" }).text, "24 December 2026, 12:00 PM");
assert.strictEqual(run("dates/now.js", { date: xmas, format: "custom", pattern: "D. MMMM", timeZone: "UTC", language: "de" }).text, "24. Dezember");
assert.strictEqual(run("dates/now.js", { date: xmas, format: "discord_F" }).text, `<t:${xmas.getTime() / 1000}:F>`);
assert.strictEqual(run("dates/now.js", { date: xmas, format: "relative" }).text, `<t:${xmas.getTime() / 1000}:R>`);
assert.strictEqual(o.unix, xmas.getTime() / 1000);
assert.throws(() => run("dates/now.js", { add: "3 fortnights" }), /Unknown time unit/);

// ---- Read Date ----
const read = (text, order) => run("dates/read_date.js", { text, order }).date;
const local = (y, mo, d, h = 0, mi = 0) => new Date(y, mo - 1, d, h, mi).getTime();
assert.strictEqual(read("2026-12-24 18:00").getTime(), local(2026, 12, 24, 18));
assert.strictEqual(read("24.12.2026").getTime(), local(2026, 12, 24));
assert.strictEqual(read("24.12.26 6:30 pm").getTime(), local(2026, 12, 24, 18, 30));
assert.strictEqual(read("01/02/2027").getTime(), local(2027, 2, 1), "day first");
assert.strictEqual(read("01/02/2027", "mdy").getTime(), local(2027, 1, 2), "month first when asked");
assert.strictEqual(read("2026-12-24T18:00:00Z").getTime(), Date.UTC(2026, 11, 24, 18));
assert.strictEqual(read("<t:1798131600:R>").getTime(), 1798131600000, "a Discord timestamp");
assert.strictEqual(read("1798131600").getTime(), 1798131600000, "unix seconds");
const before = Date.now();
const inTwo = read("in 2h").getTime() - before;
assert.ok(inTwo >= 7200000 - 50 && inTwo <= 7200000 + 1000, "in 2h");
assert.ok(Math.abs(read("1d 12h").getTime() - before - 129600000) < 1000, "a length alone is from now");
const tomorrow = read("tomorrow 9:00");
const t = new Date();
t.setDate(t.getDate() + 1);
assert.deepStrictEqual([tomorrow.getDate(), tomorrow.getHours(), tomorrow.getMinutes()], [t.getDate(), 9, 0]);
assert.ok(read("18:30") > new Date(Date.now() - 1000), "a time alone is the next time it comes");
for (const bad of ["", "soon", "31.02.2026", "25:00", "next blursday"]) assert.strictEqual(run("dates/read_date.js", { text: bad }).valid, false, bad);

// ---- Time Between ----
o = run("dates/time_between.js", { from: "2026-12-24T17:00:00Z", to: "2026-12-26T19:30:15Z" });
assert.deepStrictEqual([o.days, o.hours, o.minutes, o.text, o.past], [2, 50, 3030, "2 d 2 h 30 min", false]);
assert.strictEqual(run("dates/time_between.js", { from: xmas, to: new Date(xmas.getTime() - 90000) }).past, true);
o = run("dates/time_between.js", { from: { createdAt: new Date(Date.now() - 3 * 86400000) } });
assert.strictEqual(o.days, 3, "a user counts from when the account was made; empty To is now");

console.log("text and dates test passed");
