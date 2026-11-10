# Projects and workspaces

## A project is a folder

```
My Bot/
├─ circuit.project.json    the project: name, build mode, shared settings
├─ commands/
│  └─ rank.json            one file per slash command
├─ workspaces/
│  ├─ welcome.json         one file per workspace
│  └─ rank-command.json
└─ plugins/                optional: plugins that belong to this project
   └─ my.blocks/
```

Everything is plain, formatted JSON, so a project works well in Git: changes show as readable diffs, and two people can work on different workspaces without conflicts. The exact format is in [Project format](/api/project-format).

What's **not** shared: the bot token and other secrets (they're in the [credential store](./safety)), and what belongs to your computer alone, such as the server the bot is uploaded to, where its databases are and its stored data. Those are kept in a hidden `.circuit/` folder that stays out of Git.

## Build modes

Each project has a **build mode**, chosen in File › New Project: *Discord Bot* (discord.js) or *Fluxer Bot* (fluxer.js). The mode decides which blocks the picker offers, the categories they're sorted into, the settings in Bot › Settings, and which npm packages the bot needs. Build modes come from plugins; see [Build modes](/api/build-modes).

## Workspaces

A workspace is one canvas. A bot can have any number, sorted into folders in the Workspaces panel. They all run together: splitting a bot into workspaces is only for you.

Good habits:

- **One workspace per command or feature**: *Rank command*, *Welcome*, *Moderation*.
- **Reuse with Call.** A *When Called* event in one workspace and *Call* blocks elsewhere make a flow you can use from many places, like a function. *Return* gives a value back.
- **Signals** (Emit Signal and On Signal) let one part of the bot tell others that something happened, without waiting for them.
- **Notes** explain a part of a workspace: *Add note* in the canvas toolbar, or *Note* above selected blocks to frame them. A note has a title, a description and a colour (the pencil button, or double-click its title). Drag its title to move it with the blocks inside (hold Alt to move only the note), and its edges or corners to resize it. Notes are selected, copied, pasted, duplicated and deleted like blocks: click the title, or draw a box around the whole note.

## Starters and examples

File › New Project can start from a working bot (welcome and auto role, moderation, levels, tickets, role menus; for Fluxer: basic commands, moderation). They are ordinary projects copied into your new folder; plugins can add their own (see [Templates](/api/templates)). Help › Open Example opens DemoBot.

## Safety nets

- **Undo/redo** per workspace (Ctrl+Z / Ctrl+Y).
- **Backups**: every save keeps a snapshot outside the project (the newest 20, plus one a day for 14 days). File › Restore from backup… compares them with now and restores the whole project or one workspace.
- **Crash recovery**: unsaved changes are copied every 30 seconds (Settings › Backups changes how often) and offered back after a crash.
