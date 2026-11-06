// The Database list: the default one and each of Bot › Settings › Databases.
function databases(project) {
  const choices = { default: "Default database" };
  for (const d of (project && project.settings && project.settings.databases) || []) if (d && d.id) choices[d.id] = d.name || d.id;
  return choices;
}

module.exports = {
  id: "core.sql_query",
  kind: "action",
  info: {
    name: "SQL Query",
    description: "Runs SQL on a MariaDB, MySQL, PostgreSQL or SQLite database of Bot › Settings › Databases. Each ${…} is sent as a value, never as SQL, so what people type can't change the query.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "database",
    aliases: ["SQL", "Query", "Database query", "Run SQL", "Create table"],
    tags: ["database", "sql", "query", "select", "insert", "update", "delete", "create", "mysql", "mariadb", "postgres", "sqlite"],
  },
  look: {
    icon: "database",
  },
  fields: {
    database: { type: "select", pin: false, default: "default", choices: { default: "Default database" }, description: "Which database of Bot › Settings › Databases. Default is the one chosen there." },
    sql: {
      type: "textarea",
      pin: false,
      templateParts: true,
      required: true,
      default: "SELECT * FROM users WHERE id = ${user.id}",
      label: "SQL",
      description: "One statement. ${…} are values (a text, a number, a user's id …), not table or column names.",
    },
  },
  outputs: {
    rows: { type: "list<object>", description: "The rows a SELECT (or RETURNING) gives." },
    first: { type: "object?", description: "The first of them, or nothing." },
    count: { type: "number", description: "How many rows it gave, or changed." },
    id: { type: "any", description: "The id of a row an INSERT added, when the database says." },
  },
  define({ project }) {
    return { set: { database: { choices: databases(project) } } };
  },
  async run({ fields, ctx }) {
    const r = await ctx.db(fields.database).query(fields.sql);
    return { rows: r.rows, first: r.rows[0], count: r.rows.length || r.affected, id: r.id };
  },
};
