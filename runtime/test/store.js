// Saved data is never lost: a damaged store.json is kept aside, not overwritten.
const fs = require("fs");
const os = require("os");
const path = require("path");
const assert = require("assert");
const { Store } = require("../circuit-runtime.js");

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-store-"));
const file = path.join(dir, "store.json");
const rt = { log: null };

// A damaged file: the store starts empty and the old file stays next to it.
fs.writeFileSync(file, "{ not json");
const store = new Store(dir, rt);
assert.deepStrictEqual(store.data, {});
assert.ok(store.note && store.note.includes("damaged-"), store.note);
const kept = fs.readdirSync(dir).filter((f) => f.startsWith("store.damaged-"));
assert.strictEqual(kept.length, 1, "the damaged file was kept");
assert.strictEqual(fs.readFileSync(path.join(dir, kept[0]), "utf8"), "{ not json");

// Changes are batched, and flush writes them at once.
store.scope("global").set("level", 3);
assert.ok(!fs.existsSync(file), "not written yet: saves are batched");
store.flushPending();
assert.deepStrictEqual(JSON.parse(fs.readFileSync(file, "utf8")), { global: { level: 3 } });

// Member buckets with a ":" in an id don't run into each other; single ids
// keep the keys they always had. (Writes take effect at once; the runtime
// catches rejected promises, so this checks without awaiting.)
store.scope("member", "1:2", "3").set("xp", 1);
store.scope("member", "1", "2:3").set("xp", 2);
assert.strictEqual(store.data["member:1%3A2:3"].xp, 1);
assert.strictEqual(store.data["member:1:2%3A3"].xp, 2);
assert.deepStrictEqual(store.each("member", "1").map((b) => b.id), ["2:3"]);
store.scope("ns", "rate:limits").set("n", 1);
assert.ok("ns:rate:limits" in store.data);

// A file that is valid JSON but not an object is treated the same way.
fs.writeFileSync(file, "[1, 2]");
const again = new Store(dir, rt);
assert.deepStrictEqual(again.data, {});
assert.strictEqual(fs.readdirSync(dir).filter((f) => f.startsWith("store.damaged-")).length >= 1, true);

// Names every object answers to (constructor, toString, __proto__) are
// stored values like any other: not there until set, and kept when set.
(async () => {
  const g = store.scope("ns", "names");
  assert.strictEqual(await g.get("constructor", "fallback"), "fallback");
  assert.strictEqual(await g.has("toString"), false);
  assert.strictEqual(await g.inc("valueOf"), 1);
  assert.deepStrictEqual(await g.push("hasOwnProperty", "a"), ["a"]);
  await g.set("__proto__", { admin: true });
  assert.deepStrictEqual(await g.get("__proto__"), { admin: true });
  assert.strictEqual(await g.get("admin", "none"), "none", "what's in it isn't found under other names");
  assert.deepStrictEqual((await g.keys()).sort(), ["__proto__", "hasOwnProperty", "valueOf"]);
  store.flush();
  const saved = JSON.parse(fs.readFileSync(file, "utf8"));
  assert.deepStrictEqual(Object.keys(saved["ns:names"]).sort(), ["__proto__", "hasOwnProperty", "valueOf"], "saved, too");
  assert.strictEqual(({}).admin, undefined);
  await g.delete("__proto__");
  assert.strictEqual(await g.has("__proto__"), false);

  for (const s of [store, again]) s.flush = () => {};
  fs.rmSync(dir, { recursive: true, force: true });
  console.log("store test passed");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
