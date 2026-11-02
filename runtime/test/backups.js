// The bot's saved data is backed up when it starts (valid JSON only, newest 10
// kept), and an empty or damaged file is restored from the newest backup when
// nobody can be asked.
const fs = require("fs");
const os = require("os");
const path = require("path");
const assert = require("assert");
const { backupStore, listStoreBackups, prepareStoreFile, BACKUPS_KEPT } = require("../circuit-runtime.js");

const logs = [];
const rt = { debug: false, store: null, emit() {}, log: {} };
for (const level of ["info", "success", "warn", "error", "debug"]) rt.log[level] = (t) => logs.push(`${level}: ${t}`);

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-backups-"));
const file = path.join(dir, "store.json");

(async () => {
  // No saved data yet: nothing to back up, nothing asked.
  await prepareStoreFile(dir, rt);
  assert.strictEqual(listStoreBackups(dir).length, 0);

  // Valid data: backed up on start.
  fs.writeFileSync(file, JSON.stringify({ global: { level: 3 } }));
  await prepareStoreFile(dir, rt);
  const [first] = listStoreBackups(dir);
  assert.ok(first, "a backup was made");
  assert.deepStrictEqual(JSON.parse(fs.readFileSync(first.file, "utf8")), { global: { level: 3 } });

  // Only the newest 10 are kept.
  for (let i = 0; i < 14; i++) {
    fs.writeFileSync(file, JSON.stringify({ global: { level: 10 + i } }));
    backupStore(dir, rt);
  }
  const kept = listStoreBackups(dir);
  assert.strictEqual(kept.length, BACKUPS_KEPT);
  assert.deepStrictEqual(JSON.parse(fs.readFileSync(kept[0].file, "utf8")), { global: { level: 23 } }, "newest first");

  // A damaged file is never backed up.
  fs.writeFileSync(file, "{ broken");
  assert.strictEqual(backupStore(dir, rt), null);
  assert.strictEqual(listStoreBackups(dir).length, BACKUPS_KEPT);

  // Damaged on start: kept aside, and (nobody to ask) the newest backup comes back.
  await prepareStoreFile(dir, rt);
  assert.ok(fs.readdirSync(dir).some((n) => n.startsWith("store.damaged-")), "the damaged file was kept");
  assert.deepStrictEqual(JSON.parse(fs.readFileSync(file, "utf8")), { global: { level: 23 } });

  // Empty on start: the same.
  fs.writeFileSync(file, "   \n");
  await prepareStoreFile(dir, rt);
  assert.deepStrictEqual(JSON.parse(fs.readFileSync(file, "utf8")), { global: { level: 23 } });
  assert.ok(logs.some((l) => l.includes("Nobody can answer here")), logs.join("\n"));

  // Empty with no backups at all: the default template.
  fs.rmSync(path.join(dir, "backups"), { recursive: true, force: true });
  fs.writeFileSync(file, "");
  await prepareStoreFile(dir, rt);
  assert.deepStrictEqual(JSON.parse(fs.readFileSync(file, "utf8")), {});

  fs.rmSync(dir, { recursive: true, force: true });
  console.log("backups test passed");
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
