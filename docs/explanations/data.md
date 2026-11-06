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

## Timers: later, once

**Schedule** runs the **On Scheduled** blocks with the same name once, after a while (`30s`, `10m`, `1h 30m`, `3d`) or at a time (`2026-12-24 18:00`), with a **value** along: a text, or an object with what's needed later (`{ user, channel, text }`). For reminders, temporary bans, giveaways that end, a message that goes out on a day.

Timers are kept with the stored data, so they still run after the bot restarts. One that was due while the bot was off runs when it starts again, with **Late** saying by how much; turn off *Run missed ones* on On Scheduled to drop those instead. Users, channels and messages in the value come back as their data (id, name …), not as live objects: find the member again with its id when you need more.

- A **key** (under Advanced) replaces an earlier timer with the same key: `remind-${user.id}` gives everyone one reminder at most.
- **Cancel Scheduled** removes timers before they run: by the id Schedule gave, by key, or all with a name.
- **List Scheduled** gives the waiting timers, soonest first.

**Every** is for things that repeat (every 10 minutes, every day at 9:00); it doesn't need saving.

## In a database

Stored values can be kept in MariaDB, MySQL, PostgreSQL, SQLite or MongoDB instead of `store.json`, and the database blocks read and write your own tables there. See [Databases](./databases).

## In your own blocks

Blocks reach the same data through [`ctx.vars`](/api/ctx#vars) and [`ctx.store`](/api/ctx#store), so a custom block and the built-in ones share it.
