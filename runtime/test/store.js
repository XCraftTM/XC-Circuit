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

// A file that is valid JSON but not an object is treated the same way.
fs.writeFileSync(file, "[1, 2]");
const again = new Store(dir, rt);
assert.deepStrictEqual(again.data, {});
assert.strictEqual(fs.readdirSync(dir).filter((f) => f.startsWith("store.damaged-")).length >= 1, true);

for (const s of [store, again]) s.flush = () => {};
fs.rmSync(dir, { recursive: true, force: true });
console.log("store test passed");
