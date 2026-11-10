# Live sessions: working together

In a live session several people work in one project at the same time. One of them **hosts**: the project is theirs, it is saved on their computer, and the bot runs there. The others are **guests**: they see every change as it happens, and may change what the host lets them.

::: warning Work in progress
Sessions work for workspaces, blocks, wires, notes and slash commands; you see where the others are, and guests read the bot's console. Still to come: which blocks ran and the values on the wires for guests, the host's plugins for guests who don't have them, and pickers (channels, roles) for guests.
:::

## Starting one

1. Open the project and **File › Live session…**
2. Give yourself a name, and press **Start a session for this project**.
3. **Copy** the invitation and send it to the people you want in. It starts with `xc-live:`. Whoever has it can join, so only give it to them.

The session meets at a **relay**: the public one (`https://xcrelay.xcrafttm.dev`), or [your own host](host.md) if you put its address there. The relay only passes messages on. It can't read them: everything is sealed with a key that is part of the invitation, and so only travels from you to your guests.

## Joining one

**File › Live session…**, your name, paste the invitation, **Join**. The host's project opens. It is a copy for this session, kept in the app's own folder; it is saved at the host's, not at yours.

Host and guests need the **same version** of XC Circuit. If they differ, the guest isn't let in and is told who has to update.

## Seeing each other

Everyone has a colour. On the canvas you see the others' **pointers** with their names, in the workspace they are in, and a block someone has **selected** is framed in their colour with their name on it: so two people don't change the same block without noticing.

## What guests may do

A new guest only **watches**. The host ticks, per guest, what they may change, and can change that at any time:

| | Lets a guest |
| --- | --- |
| **The bot (start, stop)** | start and stop the bot, which runs at the host's; Run and Stop in the guest's window do it |
| **Slash commands** | make, change and delete commands |
| **Workspaces** | make, rename, move, turn on and off, and delete workspaces |
| **Blocks, everywhere** | change blocks, wires and notes in every workspace |
| **Blocks in:** (some workspaces) | the same, in the ticked workspaces only |

The host checks every change against this before taking it. A change a guest may not make is taken back at the guest's, with a word why. Bot settings, secrets, where the bot is uploaded to and plugins are the host's alone. **Send away** removes a guest.

## When several change the same thing

Changes are put together the way they are when [two stored versions meet](git.md): every setting of a block on its own, wires one by one. Two people can work in the same workspace, even on the same block.

- Where two change the **same setting** at the same moment, the **host's** stays (or, between guests, the one that reached the host first). Nobody is asked.
- Two blocks that got the same number are two blocks: the host's keeps the number, the other gets the next free one, with its wires and what reads it.
- Where a block sits is everyone's own business until they move it: a move travels like any other change.

**Undo** only takes back your own work. What the others did in between stays.

## The bot, and ending

The bot runs only at the host's: the token never leaves their computer. What it writes to the **Console** shows at every guest's too (secrets are hidden there as always), and File › Live session… says whether it runs. A guest the host ticked **The bot** for starts and stops it with their own Run and Stop; for the others, Run says that it is the host's to start.

**End the session** (the host) ends it for everyone; **Leave the session** (a guest) only for that guest. When the host's app closes or loses its connection, the session is over. A guest's copy stays open as it was, but it is only a copy: what counts is the host's project.
