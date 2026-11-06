// The Database list: the default one, each of Bot › Settings › Databases,
// and one given on the block.
function databases(project) {
  const choices = { default: "Default database" };
  for (const d of (project && project.settings && project.settings.databases) || []) if (d && d.id && d.id !== "given") choices[d.id] = d.name || d.id;
  choices.given = "Given here";
  return choices;
}

// A database given on the block (from a command's options, say). Bot ›
// Settings › Databases has to allow that.
const GIVEN_KINDS = { mariadb: "MariaDB / MySQL", postgres: "PostgreSQL", mongodb: "MongoDB" };
const GIVEN = {
  dbKind: { type: "select", pin: false, default: "mariadb", choices: GIVEN_KINDS, show: { database: ["given"] }, label: "Kind" },
  dbHost: { type: "text", default: "", show: { database: ["given"] }, label: "Host", placeholder: "db.example.com" },
  dbPort: { type: "number", default: 0, min: 0, max: 65535, show: { database: ["given"] }, label: "Port", description: "0: the kind's usual port (3306, 5432, 27017)." },
  dbUser: { type: "text", default: "", show: { database: ["given"] }, label: "User" },
  dbPassword: { type: "text", pin: "only", show: { database: ["given"] }, label: "Password", description: "Connect it from a command's option or a form (only the bot sees those): typed into a block, it would be saved in the project file." },
  dbName: { type: "text", default: "", show: { database: ["given"] }, label: "Database name" },
  dbSsl: { type: "boolean", pin: false, default: false, show: { database: ["given"] }, label: "Use SSL/TLS" },
  dbUrl: { type: "text", pin: "only", show: { database: ["given"] }, label: "Connection URL", description: "Instead of the boxes above, such as mysql://user:password@host:3306/name. Connected, like the password: it holds one." },
};

/** What ctx.db takes: a database's id, or the connection the block gives. */
function which(fields) {
  if (fields.database !== "given") return fields.database;
  return { kind: fields.dbKind, host: fields.dbHost, port: fields.dbPort, user: fields.dbUser, password: fields.dbPassword, database: fields.dbName, ssl: fields.dbSsl, url: fields.dbUrl };
}

/** A column and its value, for each of `count` columns. */
function columnFields(count) {
  const out = {};
  for (let i = 1; i <= Math.min(Number(count) || 0, 12); i++) {
    out[`column${i}`] = { type: "text", pin: false, template: false, default: "", required: true, label: `Column ${i}`, placeholder: "column", description: "The column (in MongoDB, a field)." };
    out[`column${i}Value`] = { type: "any", label: `Value ${i}`, description: "Its new value. A user, channel or server is kept as its id; lists and objects as JSON." };
  }
  return out;
}

function columnValues(fields) {
  const out = {};
  for (let i = 1; i <= Math.min(Number(fields.columns) || 0, 12); i++) {
    const name = String(fields[`column${i}`] || "").trim();
    if (name) out[name] = fields[`column${i}Value`];
  }
  if (fields.values && typeof fields.values === "object") Object.assign(out, fields.values);
  return out;
}

const COMPARE = { "=": "is", "!=": "is not", ">": "is more than", ">=": "is at least", "<": "is less than", "<=": "is at most", contains: "contains", starts: "starts with", ends: "ends with", null: "is empty", notNull: "isn't empty" };
const MAX = 8;

/** A column, how to compare and a value, for each of `count` conditions. */
function conditionFields(count) {
  const out = {};
  for (let i = 1; i <= Math.min(Number(count) || 0, MAX); i++) {
    out[`where${i}`] = { type: "text", pin: false, template: false, default: "", required: true, label: `Where ${i}`, placeholder: "column", description: "A column (in MongoDB, a field)." };
    out[`where${i}Op`] = { type: "select", pin: false, default: "=", choices: COMPARE, label: `Compare ${i}`, description: "Contains, starts with and ends with ignore upper and lower case." };
    out[`where${i}Value`] = { type: "any", label: `Value ${i}`, description: "What the column is compared with. A user, channel or server counts as its id.", show: { [`where${i}Op`]: Object.keys(COMPARE).filter((k) => k !== "null" && k !== "notNull") } };
  }
  return out;
}

function conditions(fields) {
  const out = [];
  for (let i = 1; i <= Math.min(Number(fields.conditions) || 0, MAX); i++) out.push({ column: fields[`where${i}`], op: fields[`where${i}Op`] || "=", value: fields[`where${i}Value`] });
  return out;
}

module.exports = {
  id: "core.update_rows",
  kind: "action",
  info: {
    name: "Update Rows",
    description: "Changes columns of the rows that match the conditions, in a database of Bot › Settings › Databases.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "database",
    aliases: ["Update", "Change rows", "SQL update", "Database set", "MongoDB update"],
    tags: ["database", "sql", "update", "change", "rows", "mysql", "mariadb", "postgres", "sqlite", "mongodb"],
  },
  look: {
    icon: "database",
  },
  fields: {
    database: { type: "select", pin: false, default: "default", choices: { default: "Default database" }, description: "Which database of Bot › Settings › Databases (Default is the one chosen there), or one given here, which needs its switch there." },
    ...GIVEN,
    table: { type: "text", pin: false, template: false, required: true, default: "", placeholder: "users", description: "The table (in MongoDB, the collection)." },
    columns: { type: "number", pin: false, default: 1, min: 1, max: 12, label: "Columns to change", description: "How many columns get a new value." },
    conditions: { type: "number", pin: false, default: 1, min: 0, max: MAX, description: "How many conditions a row has to match to be changed." },
    match: { type: "select", pin: false, default: "all", advanced: true, choices: { all: "All conditions", any: "Any condition" }, description: "Whether a row has to match every condition, or one is enough." },
    everyRow: { type: "boolean", pin: false, default: false, advanced: true, label: "Every row without conditions", description: "With no conditions, it only runs when this is on, so a missing condition never changes a whole table by mistake." },
    values: { type: "object", pin: "only", advanced: true, label: "More values", description: "An object of column → new value, added to the ones above." },
  },
  outputs: {
    count: { type: "number", description: "How many rows were changed." },
  },
  define({ values, project }) {
    return { set: { database: { choices: databases(project) } }, add: { ...columnFields(values.columns ?? 1), ...conditionFields(values.conditions ?? 1) } };
  },
  async run({ fields, ctx }) {
    const where = conditions(fields);
    if (!where.length && !fields.everyRow) throw new Error("Update Rows has no conditions, so it would change every row. Add a condition, or turn on Every row without conditions.");
    const count = await ctx.db(which(fields)).update(fields.table, columnValues(fields), where, fields.match === "any");
    return { count };
  },
};
