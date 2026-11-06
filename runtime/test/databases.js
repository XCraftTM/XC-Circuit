// Databases: the connection API, values kept out of the SQL, stored values
// in a database, and the database blocks running in a bot.
//
// SQLite always runs (it's built into Node.js). The others run when their
// address is given, with their npm packages in XC_TEST_DEPS (a folder whose
// node_modules has mysql2, pg and mongodb):
//   XC_TEST_MARIADB=mysql://root:pw@127.0.0.1:3306/test
//   XC_TEST_POSTGRES=postgres://postgres:pw@127.0.0.1:5432/test
//   XC_TEST_MONGODB=mongodb://127.0.0.1:27017/test
// Usage: node runtime/test/databases.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const Module = require("module");
const assert = require("assert");
const { spawn } = require("child_process");
const { Database, Databases, DbStore, templateParts } = require("../circuit-runtime.js");

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-databases-"));
const deps = process.env.XC_TEST_DEPS ? path.resolve(process.env.XC_TEST_DEPS) : dir;
const logs = [];
const log = { info: (...a) => logs.push(a.join(" ")), warn: (...a) => logs.push(a.join(" ")), error: (...a) => logs.push("ERROR " + a.join(" ")), success: () => {}, debug: () => {} };

/** A runtime with just what databases use. */
function fakeRuntime(settings, secrets = {}) {
  const rt = {
    opts: { dataDir: dir },
    log,
    require: Module.createRequire(path.join(deps, "package.json")),
    setting: (name) => settings[name],
    secret: (name) => secrets[name],
  };
  rt.databases = new Databases(rt);
  return rt;
}

/** The connections to test: SQLite, and the others when given. */
const kinds = [{ id: "lite", name: "Lite", kind: "sqlite", file: "test.sqlite" }];
const secrets = {};
for (const [kind, env] of [["mariadb", "XC_TEST_MARIADB"], ["postgres", "XC_TEST_POSTGRES"], ["mongodb", "XC_TEST_MONGODB"]]) {
  if (!process.env[env]) continue;
  kinds.push({ id: kind, name: kind, kind });
  secrets[`db_${kind}_url`] = process.env[env];
}

const KEY = { sqlite: "INTEGER PRIMARY KEY AUTOINCREMENT", postgres: "SERIAL PRIMARY KEY", mariadb: "INT AUTO_INCREMENT PRIMARY KEY" };
const TEXT = { sqlite: "TEXT", postgres: "TEXT", mariadb: "VARCHAR(200)" };

/** The same checks on every kind of database. */
async function exercise(db) {
  const kind = db.kind;
  if (db.sql) {
    await db.query("DROP TABLE IF EXISTS xc_test_users");
    await db.query(`CREATE TABLE xc_test_users (id ${KEY[kind]}, name ${TEXT[kind]}, xp INTEGER, guild ${TEXT[kind]})`);
  } else {
    await db.connect();
    await db.db.collection("xc_test_users").deleteMany({});
  }
  const a = await db.insert("xc_test_users", { name: "Alex", xp: 50, guild: "g1" });
  assert.ok(a.id !== undefined && a.id !== null, `${kind}: an id for the new row`);
  await db.insert("xc_test_users", { name: "Bea", xp: 120, guild: "g1" });
  await db.insert("xc_test_users", { name: "Cem", xp: 80, guild: "g2" });
  // A Discord-like object counts as its id.
  class Guild {
    constructor(id) {
      this.id = id;
    }
  }
  await db.insert("xc_test_users", { name: "Dana", xp: null, guild: new Guild("g3") });

  const names = (rows) => rows.map((r) => r.name).sort();
  assert.deepStrictEqual(names(await db.select("xc_test_users", [{ column: "guild", op: "=", value: "g1" }])), ["Alex", "Bea"], `${kind}: =`);
  assert.deepStrictEqual(names(await db.select("xc_test_users", [{ column: "guild", op: "=", value: new Guild("g3") }])), ["Dana"], `${kind}: an object compares as its id`);
  assert.deepStrictEqual(names(await db.select("xc_test_users", [{ column: "xp", op: ">", value: 60 }])), ["Bea", "Cem"], `${kind}: >`);
  assert.deepStrictEqual(names(await db.select("xc_test_users", [{ column: "name", op: "contains", value: "EA" }])), ["Bea"], `${kind}: contains ignores case`);
  assert.deepStrictEqual(names(await db.select("xc_test_users", [{ column: "name", op: "starts", value: "a" }])), ["Alex"], `${kind}: starts with`);
  assert.deepStrictEqual(names(await db.select("xc_test_users", [{ column: "name", op: "contains", value: "%" }])), [], `${kind}: % is a character, not a wildcard`);
  assert.deepStrictEqual(names(await db.select("xc_test_users", [{ column: "xp", op: "null" }])), ["Dana"], `${kind}: is empty`);
  assert.deepStrictEqual(names(await db.select("xc_test_users", [{ column: "guild", op: "=", value: "g2" }, { column: "xp", op: ">=", value: 120 }], { any: true })), ["Bea", "Cem"], `${kind}: any condition`);
  const top = await db.select("xc_test_users", [{ column: "xp", op: "notNull" }], { order: "xp", desc: true, limit: 2 });
  assert.deepStrictEqual(top.map((r) => r.name), ["Bea", "Cem"], `${kind}: sorted, at most 2`);
  const page = await db.select("xc_test_users", [{ column: "xp", op: "notNull" }], { order: "xp", desc: true, limit: 2, offset: 2 });
  assert.deepStrictEqual(page.map((r) => r.name), ["Alex"], `${kind}: skip`);
  assert.strictEqual(typeof top[0].xp, "number", `${kind}: numbers come back as numbers`);

  assert.strictEqual(await db.update("xc_test_users", { xp: 999 }, [{ column: "guild", op: "=", value: "g1" }]), 2, `${kind}: update count`);
  assert.deepStrictEqual(names(await db.select("xc_test_users", [{ column: "xp", op: "=", value: 999 }])), ["Alex", "Bea"]);
  assert.strictEqual(await db.remove("xc_test_users", [{ column: "name", op: "=", value: "Cem" }]), 1, `${kind}: delete count`);
  assert.strictEqual((await db.select("xc_test_users")).length, 3);

  // Names are checked; values never become SQL (or MongoDB operators).
  // (In MongoDB any text is a collection name and harmless; only $ starts an operator.)
  if (db.sql) await assert.rejects(() => db.select("xc_test_users; DROP TABLE xc_test_users"), /can't be a/);
  else await assert.rejects(() => db.select("$where"), /can't be a/);
  await assert.rejects(() => db.select("xc_test_users", [{ column: db.sql ? "name; --" : "$where", op: "=", value: 1 }]), /can't be a/);
  const evil = "x' OR '1'='1";
  assert.deepStrictEqual(await db.select("xc_test_users", [{ column: "name", op: "=", value: evil }]), [], `${kind}: a quote in a value is just a quote`);
  assert.deepStrictEqual(await db.select("xc_test_users", [{ column: "name", op: "=", value: { $ne: null } }]), [], `${kind}: an object value isn't an operator`);
  if (db.sql) {
    const parts = templateParts("SELECT name FROM xc_test_users WHERE name = ${who} ORDER BY name", null, { who: "Bea'; DROP TABLE xc_test_users; --" });
    assert.deepStrictEqual(parts.values, ["Bea'; DROP TABLE xc_test_users; --"]);
    assert.deepStrictEqual((await db.query(parts)).rows, []);
    const ok = await db.query(templateParts("SELECT name FROM xc_test_users WHERE name = ${who}", null, { who: "Bea" }));
    assert.deepStrictEqual(ok.rows, [{ name: "Bea" }], `${kind}: \${…} as a parameter`);
    assert.strictEqual((await db.select("xc_test_users")).length, 3, `${kind}: the table is still there`);
    await db.query("DROP TABLE xc_test_users");
  } else {
    await assert.rejects(() => db.query("SELECT 1"), /MongoDB/);
    await db.db.collection("xc_test_users").drop();
  }
  const info = await db.describe();
  assert.ok(info.version, `${kind}: describe gives a version`);
}

/** Stored values (Set/Get Stored Value) in a database. */
async function storeIn(spec) {
  const rt = fakeRuntime({ databases: [spec], storeIn: spec.id }, secrets);
  const db = rt.databases.get(spec.id);
  if (db.sql) await db.query(`DROP TABLE IF EXISTS xc_store`);
  else {
    await db.connect();
    await db.db.collection("xc_store").deleteMany({});
  }
  // The first time, store.json's values are copied in.
  fs.writeFileSync(path.join(dir, "store.json"), JSON.stringify({ "user:1": { xp: 5, name: "Alex" }, global: { level: 2 } }));
  const store = await DbStore.open(rt, spec.id);
  assert.ok(store.note && store.note.includes("copied 3"), store.note);
  const api = store.api();
  assert.strictEqual(await api.user("1").get("xp"), 5);
  await api.user("1").inc("xp", 10);
  await api.user("2").set("tags", ["a", "b"]);
  await api.user("1").delete("name");
  await api.member("g:1", "u").set("n", 1);
  await store.flushAll();
  // A new bot reads them back, and doesn't copy store.json again.
  const again = await DbStore.open(fakeRuntime({ databases: [spec], storeIn: spec.id }, secrets), spec.id);
  assert.strictEqual(again.note, null);
  assert.deepStrictEqual(again.data["user:1"], { xp: 15 });
  assert.deepStrictEqual(again.data["user:2"], { tags: ["a", "b"] });
  assert.deepStrictEqual(again.data.global, { level: 2 });
  assert.deepStrictEqual(again.each("member", "g:1").map((b) => b.id), ["u"]);
  for (const r of [rt, ...[...openStoresOf(again)]]) await r.databases?.closeAll();
  await again.db.close();
  fs.rmSync(path.join(dir, "store.json"), { force: true });
}

function openStoresOf() {
  return [];
}

/** Which database "default" means. */
function defaults() {
  const two = [
    { id: "a", name: "First", kind: "sqlite", file: "a.sqlite" },
    { id: "b", name: "Second", kind: "sqlite", file: "b.sqlite" },
  ];
  assert.throws(() => fakeRuntime({}).databases.get(), /No database is set up/);
  assert.throws(() => fakeRuntime({ databases: two }).databases.get("default"), /No default database/);
  assert.strictEqual(fakeRuntime({ databases: [two[0]] }).databases.get().spec.id, "a", "the only one is the default");
  assert.strictEqual(fakeRuntime({ databases: two, databaseDefault: "b" }).databases.get("default").spec.id, "b");
  assert.strictEqual(fakeRuntime({ databases: two }).databases.get("Second").spec.id, "b", "by name too");
  assert.throws(() => fakeRuntime({ databases: two }).databases.get("nope"), /no database "nope"/);
}

/** The blocks in a running bot, on SQLite, with stored values there too. */
function blocksInABot() {
  const root = path.join(__dirname, "..", "..");
  const blocks = {};
  for (const sub of ["database", "events", "debug", "data"]) {
    const folder = path.join(root, "plugins", "xc.core", "blocks", sub);
    for (const f of fs.readdirSync(folder)) {
      const src = fs.readFileSync(path.join(folder, f), "utf8");
      blocks[/id:\s*"([^"]+)"/.exec(src)[1]] = { source: src, file: path.join(folder, f) };
    }
  }
  const n = (block, values = {}) => ({ block: block + "@1", at: [0, 0], values });
  const say = (message) => n("core.log", { message, level: "info" });
  const chain = ["start", "create", "ins1", "ins2", "get", "getLog", "upd", "updLog", "del", "delLog", "set", "count", "countLog", "hack", "hackLog", "none"];
  const nodes = {
    start: n("core.on_start"),
    create: n("core.sql_query", { sql: "CREATE TABLE IF NOT EXISTS levels (user TEXT PRIMARY KEY, xp INTEGER)" }),
    ins1: n("core.insert_row", { table: "levels", columns: 2, column1: "user", column1Value: "u1", column2: "xp", column2Value: 10 }),
    ins2: n("core.insert_row", { table: "levels", columns: 2, column1: "user", column1Value: "u2", column2: "xp", column2Value: 30 }),
    get: n("core.get_rows", { table: "levels", conditions: 1, where1: "xp", where1Op: ">", where1Value: 5, orderBy: "xp", descending: true }),
    getLog: say("rows ${#get.count} first ${#get.first.user}"),
    upd: n("core.update_rows", { table: "levels", columns: 1, column1: "xp", column1Value: 99, conditions: 1, where1: "user", where1Op: "=", where1Value: "u1" }),
    updLog: say("updated ${#upd.count}"),
    del: n("core.delete_rows", { database: "main", table: "levels", conditions: 1, where1: "user", where1Op: "=", where1Value: "u2" }),
    delLog: say("deleted ${#del.count}"),
    set: n("core.set_stored", { scope: "global", key: "runs", value: 1 }),
    count: n("core.sql_query", { sql: "SELECT COUNT(*) AS n, MAX(xp) AS top FROM levels WHERE user = ${'u1'}" }),
    countLog: say("left ${#count.first.n} top ${#count.first.top}"),
    hack: n("core.get_rows", { table: "levels", conditions: 1, where1: "user", where1Op: "=", where1Value: "u1' OR '1'='1" }),
    hackLog: say("hack ${#hack.count}"),
    none: n("core.delete_rows", { table: "levels", conditions: 0 }),
  };
  const links = [];
  for (let i = 0; i + 1 < chain.length; i++) links.push(`${chain[i]}.next > ${chain[i + 1]}.in`);
  const bundle = {
    project: {
      name: "db",
      mode: "none",
      settings: { databases: [{ id: "main", name: "Main", kind: "sqlite", file: "bot.sqlite" }], databaseDefault: "main", storeIn: "main" },
      data: {},
    },
    blocks,
    libs: [],
    workspaces: { main: { name: "Main", nodes, links } },
  };
  const botDir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-db-bot-"));
  const file = path.join(botDir, "bundle.json");
  fs.writeFileSync(file, JSON.stringify(bundle));
  const runtime = path.join(__dirname, "..", "circuit-runtime.js");
  const child = spawn(process.execPath, [runtime, "--bundle", file, "--deps", botDir, "--data", path.join(botDir, "data"), "--debug", "--stop-on-input"], { stdio: ["pipe", "pipe", "pipe"] });
  const seen = [];
  let out = "";
  child.stdout.on("data", (d) => {
    out += d;
    for (const line of out.split("\n").slice(0, -1)) {
      if (!line.startsWith("@@xc ")) continue;
      const e = JSON.parse(line.slice(5));
      if (e.t === "log") seen.push(`${e.level}: ${e.text}`);
    }
    out = out.slice(out.lastIndexOf("\n") + 1);
  });
  let err = "";
  child.stderr.on("data", (d) => (err += d));
  // It runs until told to stop, and then writes what's waiting before it ends.
  const exited = new Promise((r) => child.on("exit", r));
  const want = ["rows 2 first u2", "updated 1", "deleted 1", "left 1 top 99", "hack 0"];
  return new Promise((resolve, reject) => {
    const started = Date.now();
    const timer = setInterval(() => {
      const done = want.every((w) => seen.some((s) => s.endsWith(w))) && seen.some((s) => s.includes("would delete every row"));
      if (!done && Date.now() - started < 20000) return;
      clearInterval(timer);
      child.stdin.write("stop\n");
      exited.then(() => {
        try {
          for (const w of want) assert.ok(seen.some((s) => s.endsWith(w)), `bot logged "${w}":\n${seen.join("\n")}\n${err}`);
          assert.ok(seen.some((s) => s.includes("would delete every row")), "Delete Rows without conditions refuses");
          // Stored values went to the database, and were written before the bot ended.
          const { DatabaseSync } = require("node:sqlite");
          const sqlite = new DatabaseSync(path.join(botDir, "data", "bot.sqlite"));
          const rows = sqlite.prepare("SELECT bucket, name, value FROM xc_store").all();
          sqlite.close();
          assert.deepStrictEqual(rows.map((r) => ({ ...r })), [{ bucket: "global", name: "runs", value: "1" }]);
          assert.ok(!err.includes("ExperimentalWarning"), `no SQLite warning: ${err}`);
          fs.rmSync(botDir, { recursive: true, force: true });
          resolve();
        } catch (e) {
          reject(e);
        }
      });
    }, 100);
  });
}

/** Databases given on a block: only when allowed, kept open by their details, never named by their password. */
async function given() {
  const off = fakeRuntime({});
  assert.throws(() => off.databases.get({ kind: "mariadb", host: "db.example.com" }), /Databases given on a block are off/);
  const rt = fakeRuntime({ databaseGivenAllowed: true });
  assert.throws(() => rt.databases.get({ kind: "sqlite", host: "x" }), /MariaDB \/ MySQL, PostgreSQL or MongoDB/);
  assert.throws(() => rt.databases.get({ kind: "postgres", password: "pw" }), /no host and no connection URL/);
  const a = rt.databases.get({ kind: "postgres", host: "db.example.com", port: 5433, user: "bot", password: "hunter2" });
  assert.strictEqual(a, rt.databases.get({ kind: "postgres", host: "db.example.com", port: "5433", user: "bot", password: "hunter2" }));
  assert.notStrictEqual(a, rt.databases.get({ kind: "postgres", host: "db.example.com", port: 5433, user: "bot", password: "other" }));
  assert.strictEqual(a.secret("password"), "hunter2");
  assert.strictEqual(a.name, "db.example.com:5433");
  const viaUrl = rt.databases.get({ kind: "mariadb", url: "mysql://bot:hunter2@db.example.com/x" });
  assert.ok(!viaUrl.name.includes("hunter2") && !viaUrl.name.includes("mysql://"), viaUrl.name);
  // At most MAX_GIVEN stay open: the least recently used goes.
  for (let i = 0; i < 12; i++) rt.databases.get({ kind: "mariadb", host: `h${i}.example.com` });
  assert.strictEqual(rt.databases.givenOpen.length, 8);
  assert.ok(!rt.databases.givenOpen.some((d) => d.spec.host === "h0.example.com"), "the least recently used closed");
  // A block set to Given here hands ctx.db the connection, not an id.
  let handed;
  const ctx = { db: (w) => ((handed = w), { select: async () => [] }) };
  await require("../../plugins/xc.core/blocks/database/get_rows.js").run({ fields: { database: "given", dbKind: "postgres", dbHost: "h", dbPort: 0, dbUser: "u", dbPassword: "p", dbName: "n", dbSsl: true, table: "t", conditions: 0 }, ctx });
  assert.deepStrictEqual(handed, { kind: "postgres", host: "h", port: 0, user: "u", password: "p", database: "n", ssl: true, url: undefined });
  await require("../../plugins/xc.core/blocks/database/get_rows.js").run({ fields: { database: "db1", table: "t", conditions: 0 }, ctx });
  assert.strictEqual(handed, "db1");
  // And a real one, given by its URL, for each server here.
  for (const spec of kinds.filter((k) => k.kind !== "sqlite")) {
    const db = rt.databases.get({ kind: spec.kind, url: secrets[`db_${spec.kind}_url`] });
    assert.ok(await db.describe(), spec.kind);
  }
  await rt.databases.closeAll();
}

(async () => {
  defaults();
  await given();
  console.log("  given on a block: ok");
  for (const spec of kinds) {
    const rt = fakeRuntime({ databases: [spec] }, secrets);
    const db = rt.databases.get(spec.id);
    await exercise(db);
    await db.close();
    await storeIn(spec);
    console.log(`  ${spec.kind}: ok`);
  }
  await blocksInABot();
  console.log("  blocks in a bot: ok");
  fs.rmSync(dir, { recursive: true, force: true });
  console.log("databases test passed");
  process.exit(0);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
