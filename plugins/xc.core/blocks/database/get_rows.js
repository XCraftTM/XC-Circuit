// The Database list: the default one and each of Bot › Settings › Databases.
function databases(project) {
  const choices = { default: "Default database" };
  for (const d of (project && project.settings && project.settings.databases) || []) if (d && d.id) choices[d.id] = d.name || d.id;
  return choices;
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
  id: "core.get_rows",
  kind: "action",
  info: {
    name: "Get Rows",
    description: "Reads the rows of a table that match the conditions, from a database of Bot › Settings › Databases (MariaDB, MySQL, PostgreSQL, SQLite or MongoDB).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "database",
    aliases: ["Select", "Database query", "Find rows", "SQL select", "MySQL", "MariaDB", "PostgreSQL", "SQLite", "MongoDB find"],
    tags: ["database", "sql", "select", "query", "rows", "mysql", "mariadb", "postgres", "sqlite", "mongodb"],
  },
  look: {
    icon: "database",
  },
  fields: {
    database: { type: "select", pin: false, default: "default", choices: { default: "Default database" }, description: "Which database of Bot › Settings › Databases. Default is the one chosen there." },
    table: { type: "text", pin: false, template: false, required: true, default: "", placeholder: "users", description: "The table (in MongoDB, the collection)." },
    conditions: { type: "number", pin: false, default: 1, min: 0, max: MAX, description: "How many conditions a row has to match. 0 gives every row." },
    match: { type: "select", pin: false, default: "all", advanced: true, choices: { all: "All conditions", any: "Any condition" }, description: "Whether a row has to match every condition, or one is enough." },
    orderBy: { type: "text", pin: false, template: false, default: "", advanced: true, label: "Sort by", placeholder: "column", description: "A column to sort by. Empty: the database's order." },
    descending: { type: "boolean", pin: false, default: false, advanced: true, description: "Largest first (or Z to A)." },
    limit: { type: "number", default: 0, min: 0, advanced: true, label: "At most", description: "How many rows at most. 0: all of them." },
    offset: { type: "number", default: 0, min: 0, advanced: true, label: "Skip", description: "Leaves out this many rows first, for pages." },
  },
  outputs: {
    rows: { type: "list<object>", description: "The rows: objects with a value for each column." },
    first: { type: "object?", description: "The first row, or nothing when none match." },
    count: { type: "number", description: "How many rows there are." },
  },
  define({ values, project }) {
    return { set: { database: { choices: databases(project) } }, add: conditionFields(values.conditions ?? 1) };
  },
  async run({ fields, ctx }) {
    const rows = await ctx.db(fields.database).select(fields.table, conditions(fields), {
      any: fields.match === "any",
      order: String(fields.orderBy || "").trim() || undefined,
      desc: fields.descending,
      limit: fields.limit,
      offset: fields.offset,
    });
    return { rows, first: rows[0], count: rows.length };
  },
};
