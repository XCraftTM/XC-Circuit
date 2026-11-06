// The Database list: the default one and each of Bot › Settings › Databases.
function databases(project) {
  const choices = { default: "Default database" };
  for (const d of (project && project.settings && project.settings.databases) || []) if (d && d.id) choices[d.id] = d.name || d.id;
  return choices;
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
    version: "1.0.0",
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
    database: { type: "select", pin: false, default: "default", choices: { default: "Default database" }, description: "Which database of Bot › Settings › Databases. Default is the one chosen there." },
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
    const { id, row } = await ctx.db(fields.database).insert(fields.table, columnValues(fields));
    return { id, row };
  },
};
