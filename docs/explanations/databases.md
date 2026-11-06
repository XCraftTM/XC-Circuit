# Databases

::: info In short
Add a database in **Bot › Settings › Databases** (MariaDB, MySQL, PostgreSQL, SQLite or MongoDB), then read and write it with **Get Rows**, **Insert Row**, **Update Rows**, **Delete Rows** and **SQL Query**. Stored values can live there too.
:::

[Stored data](./data) is enough for most bots. A database is worth it when the data gets big, when another program (a website, a dashboard) reads it too, or when you have one already.

## Adding a database

**Bot › Settings › Databases › + Add database**, then:

| Kind | What it needs |
| --- | --- |
| **MariaDB / MySQL**, **PostgreSQL** | Host, port, user, password and the database's name. Turn on SSL/TLS for databases on the internet. |
| **SQLite** | Only a file name, such as `bot.sqlite`: it's made in the bot's data folder the first time. Nothing to install. |
| **MongoDB** | Host and port, or a connection URL such as the `mongodb+srv://…` MongoDB Atlas gives. |

Instead of host, port, user and password, any kind can take a **connection URL**: `mysql://user:password@host:3306/name`, `postgres://…`, `mongodb+srv://…`.

**Test connection** connects the way the bot will and shows the server's version and its tables, or what went wrong: a wrong password, a host that isn't found, a port nothing answers on.

Passwords and connection URLs are kept in the system's password store, like the bot token, never in the project. The rest (host, user, database name) is in the project.

### The default database

One database is the **default**. Every database block has a **Database** list: *Default database*, or one by name. Blocks left on Default all switch when you choose another default, so a bot can move from a test database to the real one in one place.

### A database given on the block

**Database** can also be *Given here*: the block connects to a MariaDB / MySQL, PostgreSQL or MongoDB server it's given, such as one from a slash command's options. Kind, host, port, user, database name and SSL are fields you can type or connect; the **password** and the **connection URL** (which holds one) can only be connected: typed into a block they'd be saved in the project file.

This is off until you allow it: Bot › Settings › Databases › Safety › *Allow databases given on blocks*. Anyone who can use such a command can make the bot connect where they say (to machines on your own network, too), so only turn it on for commands you trust them with.

The bot keeps up to 8 given connections open for the next run with the same details, and closes the one used longest ago after that. An export gets the npm package of each kind chosen on a block.

## The blocks

| Block | What it does |
| --- | --- |
| **Get Rows** | The rows of a table that match the conditions: all of them (`rows`), the first one, and how many. In the Inspector: sort, at most, skip (for pages). |
| **Insert Row** | Adds a row. Gives the new row's id (an auto-increment column, or MongoDB's `_id`). |
| **Update Rows** | Changes columns of the rows that match. Gives how many. |
| **Delete Rows** | Deletes the rows that match. Gives how many. |
| **SQL Query** | Runs your own SQL (not MongoDB). |

**Conditions** are a column, a comparison (*is*, *is not*, *is more than*, *contains*, *starts with*, *is empty* …) and a value; a row has to match all of them (or, in the Inspector, any one). The number on the block says how many there are.

Update Rows and Delete Rows without any condition would change or delete every row, so they refuse unless **Every row without conditions** is turned on in the Inspector.

A user, channel or server given as a value counts as its **id**. Lists and objects are kept as JSON (in MongoDB, as they are). Keep Discord ids in text columns (`VARCHAR`, `TEXT`): they're too big for exact numbers in some databases.

In MongoDB, a *table* is a collection and a *column* is a field.

### SQL Query and `${…}`

```sql
SELECT xp, level FROM levels WHERE user_id = ${user.id} AND guild_id = ${server.id}
```

Each `${…}` is sent to the database as a **value**, separately from the SQL. So whatever people type into a command can't change the query (no SQL injection), and quotes need no escaping. That's also why `${…}` can't be a table or column name.

The tables have to exist before the bot uses them; SQL Query can make them:

```sql
CREATE TABLE IF NOT EXISTS levels (user_id VARCHAR(20) PRIMARY KEY, xp INTEGER DEFAULT 0)
```

## Stored values in a database

**Keep stored values in** (on the same page) moves Set Stored Value, Get Stored Value, Increase, Leaderboard and the rest into a database, in a table `xc_store` the bot makes. The blocks don't change.

The first time, the bot copies what's in `store.json` into the database (when its table is still empty); `store.json` stays as it was. The Stored Data view shows and changes the values there, and each database's tables too. Backups of stored data are for `store.json` only: back up a database with its own tools.

## Exported bots

An export gets the database's npm package (`mysql2`, `pg` or `mongodb`) in its `package.json`. Passwords and URLs go to `.env`, as `XC_SECRET_DB_<ID>_PASSWORD` and `XC_SECRET_DB_<ID>_URL` (with *Include the bot token and secrets*); the id is the database's, such as `DB1`. A SQLite file is in the export's `data/` folder.

## In your own blocks

[`ctx.db()`](/api/ctx#db) is the same connection the built-in blocks use.
