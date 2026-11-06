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

module.exports = {
  id: "core.sql_query",
  kind: "action",
  info: {
    name: "SQL Query",
    description: "Runs SQL on a MariaDB, MySQL, PostgreSQL or SQLite database of Bot › Settings › Databases. Each ${…} is sent as a value, never as SQL, so what people type can't change the query.",
    author: "XC Team",
    version: "1.1.0",
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
    database: { type: "select", pin: false, default: "default", choices: { default: "Default database" }, description: "Which database of Bot › Settings › Databases (Default is the one chosen there), or one given here, which needs its switch there." },
    ...GIVEN,
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
    const r = await ctx.db(which(fields)).query(fields.sql);
    return { rows: r.rows, first: r.rows[0], count: r.rows.length || r.affected, id: r.id };
  },
};
