# Variables and stored data

Bots need to remember things: for a moment, while a command runs, or for months. XC™ Circuit has two tools for that.

## Variables: while the bot runs

Variables are set with **Set Variable** and read with **Get Variable** (or by name in [`${…}`](./expressions)). They live in memory and come in three scopes:

| Scope | Lives for | Use it for |
| --- | --- | --- |
| **Run** | one run, from its event to its end | a value you work out early and need later in the same flow |
| **Workspace** | as long as the bot runs, shared by the runs of one workspace | a cache, a queue for one feature |
| **Global** | as long as the bot runs, shared by everything | state the whole bot needs |

Variables are gone when the bot stops.

## Stored data: across restarts

**Set, Get, Increase and Delete Stored Value**, **List Stored Keys** and **Leaderboard** keep data in a file, so it survives restarts. Each value has a **key** (such as `xp`) and belongs to a scope:

| Scope | One value per… |
| --- | --- |
| Whole bot | bot |
| Server | server |
| User | user, in every server |
| Member | user in one server |
| Channel | channel |

So `xp` for *Member* gives everyone their own XP in each server, while `xp` for *User* follows a person across servers.

The data is kept in `store.json`: in the project's `.circuit/data/` folder while the bot runs from the editor, and in `data/` next to an exported bot. While you work in the editor, the **Stored Data** view in the side bar shows it.

## Backups of bot data

A bot backs up its stored data when it starts and every 6 hours, keeping the newest 10 in a `backups` folder next to `store.json`. If the data is empty or damaged at start, the bot asks whether to restore a backup or start empty (a dialog in XC Circuit, a question in the terminal for exported bots).

## In a database

Stored values can be kept in MariaDB, MySQL, PostgreSQL, SQLite or MongoDB instead of `store.json`, and the database blocks read and write your own tables there. See [Databases](./databases).

## In your own blocks

Blocks reach the same data through [`ctx.vars`](/api/ctx#vars) and [`ctx.store`](/api/ctx#store), so a custom block and the built-in ones share it.
