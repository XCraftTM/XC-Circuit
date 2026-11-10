# Live sessions: working together

In a live session several people work in one project at the same time. One of them **hosts**: the project is theirs, it is saved on their computer, and the bot runs there. The others are **guests**: they see every change as it happens, and may change what the host lets them.

::: warning New
Live sessions are new in 0.3.0. Stored Data and where the bot is uploaded to stay with the host; a guest doesn't see them.
:::

## Starting one

1. Open the project and **File › Live session…**
2. Give yourself a name, and press **Start a session for this project**.
3. **Copy** the invitation and send it to the people you want in. It starts with `xc-live:`. Whoever has it can join, so only give it to them.

The session meets at a **relay**: the public one (`https://xcrelay.xcrafttm.dev`), or [your own host](host.md) if you put its address there. The relay only passes messages on. It can't read them: everything is sealed with a key that is part of the invitation, and so only travels from you to your guests.

## Joining one

**File › Live session…**, your name, paste the invitation, **Join**. The host's project opens. It is a copy for this session, kept in the app's own folder; it is saved at the host's, not at yours.

Host and guests need the **same version** of XC Circuit. If they differ, the guest isn't let in and is told who has to update.

### Plugins the host has and you don't

The plugins that come with XC Circuit are the same for everyone. Others the host uses (ones they installed, or the project's own) are passed on to you, into your copy of the project: they never replace a plugin of your own. They are someone else's code, so XC Circuit **asks before it loads them**, as with any project that brings plugins along. Say no and the session still works; blocks from those plugins then show as unknown. A plugin of more than 4 MB isn't passed on.

### When your connection drops

Your app tries to get back in by itself, for about two minutes. Back in, you are who you were, with what the host let you do; your copy catches up with what happened meanwhile, and what you changed while away is sent after all. If the host's own connection drops, the session is over.

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
| **Servers, channels, roles** | pick from the bot's servers, channels and roles in a block's fields; without it a field only shows the id that is in it |
| **Values on wires** | see what went through the wires when the bot ran: real members' names and messages |

The host checks every change against this before taking it. A change a guest may not make is taken back at the guest's, with a word why. Bot settings, secrets, where the bot is uploaded to and plugins are the host's alone. **Send away** removes a guest.

## When several change the same thing

Changes are put together the way they are when [two stored versions meet](git.md): every setting of a block on its own, wires one by one. Two people can work in the same workspace, even on the same block.

- Where two change the **same setting** at the same moment, the **host's** stays (or, between guests, the one that reached the host first). Nobody is asked.
- Two blocks that got the same number are two blocks: the host's keeps the number, the other gets the next free one, with its wires and what reads it.
- Where a block sits is everyone's own business until they move it: a move travels like any other change.

**Undo** only takes back your own work. What the others did in between stays.

## The bot, and ending

The bot runs only at the host's: the token never leaves their computer. What it writes to the **Console** shows at every guest's too (secrets are hidden there as always), and File › Live session… says whether it runs. A guest the host ticked **The bot** for starts and stops it with their own Run and Stop; for the others, Run says that it is the host's to start.

Every guest sees **which blocks ran**, how long they took, and where one failed, lit up on the canvas as at the host's. The values on the wires are for guests with that tick only.

## What the bot's token is for

A guest has no token. Whatever needs it, the host's app reads and passes on, and only what it shows itself: how the bot looks (its name and picture, for the previews: every guest gets that, so previews look the same for all) and, for guests with the tick, the list of servers with their channels and roles. A guest can't ask Discord for anything else through the host, and nothing on this way changes anything at Discord.

**End the session** (the host) ends it for everyone; **Leave the session** (a guest) only for that guest. When the host's app closes or loses its connection, the session is over. A guest's copy stays open as it was, but it is only a copy: what counts is the host's project.
