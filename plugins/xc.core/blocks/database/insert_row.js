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
    out[`column${i}Value`] = { type: "any", label: `Value ${i}`, description: "Its value. A user, channel or server is kept as its id; lists and objects as JSON." };
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

module.exports = {
  id: "core.insert_row",
  kind: "action",
  info: {
    name: "Insert Row",
    description: "Adds a row to a table of a database in Bot › Settings › Databases (in MongoDB, a document to a collection).",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "database",
    aliases: ["Insert", "Add row", "SQL insert", "Database add", "MongoDB insert"],
    tags: ["database", "sql", "insert", "add", "row", "mysql", "mariadb", "postgres", "sqlite", "mongodb"],
  },
  look: {
    icon: "database",
  },
  fields: {
    database: { type: "select", pin: false, default: "default", choices: { default: "Default database" }, description: "Which database of Bot › Settings › Databases (Default is the one chosen there), or one given here, which needs its switch there." },
    ...GIVEN,
    table: { type: "text", pin: false, template: false, required: true, default: "", placeholder: "users", description: "The table (in MongoDB, the collection)." },
    columns: { type: "number", pin: false, default: 2, min: 1, max: 12, description: "How many columns to fill in." },
    values: { type: "object", pin: "only", advanced: true, label: "More values", description: "An object of column → value, added to the ones above." },
  },
  outputs: {
    id: { type: "any", description: "The new row's id: an auto-increment id column, or MongoDB's _id." },
    row: { type: "object?", description: "The new row, as the database keeps it (not for MariaDB and MySQL)." },
  },
  define({ values, project }) {
    return { set: { database: { choices: databases(project) } }, add: columnFields(values.columns ?? 2) };
  },
  async run({ fields, ctx }) {
    const { id, row } = await ctx.db(which(fields)).insert(fields.table, columnValues(fields));
    return { id, row };
  },
};
