# Blocks for complete Discord bots

What a bot built in XC Circuit can do, as blocks. Everything planned here is built (✅);
the few things that aren't are listed under "Not built yet". Block names in the editor may
differ slightly from this list (Call Workspace is **Call**, Set Bot Name / Avatar is **Set Bot
Profile**, Voice: Move / Disconnect … is **Voice Member**).

For anything the blocks don't cover, two blocks reach everything discord.js can do:

- **Run Code** runs your own JavaScript with named inputs and outputs.
- **Use Method** calls any method, or reads or sets any property, of any object or of the
  Discord client, with typed or connected arguments.

Blocks fall into three kinds: **events** start a flow, **actions** do something in order
(green wires), **values** compute something when an action needs it. Every text field takes
JavaScript in `${…}`; names in it get an input to connect a value to unless the event,
variables or earlier blocks give them.

## What bots are built from

The blocks below are chosen so these common bots can be built without code:

| Bot feature | Built from |
|---|---|
| Moderation | Slash Command, Check Permission, Moderate Member, Bulk Delete, Lock Channel, Set Slowmode, stored warnings, Audit Log |
| Welcome and goodbye | Member Joins/Leaves, Send Message (to a set channel), Build Embed, Add Role (auto role) |
| Reaction, button and menu roles | Reaction Added/Removed, Button Pressed, Select Menu Used, Add/Remove Role |
| Levels and XP | Message Sent, Cooldown, Increase Stored Value, Leaderboard, Build Embed |
| Tickets | Button Pressed, Create Channel or Create Thread, Set Channel Permissions, Close with transcript (Get Messages) |
| Polls and giveaways | Create Poll, buttons with custom ids, Every (timer), Random item, Edit Message |
| Logs | Message Edited/Deleted, Member Updated, Banned/Unbanned, Channel and Role events |
| Verification | Button Pressed, Show Modal, Modal Submitted, Add Role |
| Suggestions and starboard | Message Sent, Add Reaction, Create Thread, Reaction Added (count) |
| Economy and daily rewards | Cooldown, stored balances, Leaderboard, Select Menu shop |
| Reminders and scheduled posts | Every / At time, stored reminders, Send Message, Send DM |
| Temporary voice channels | Voice State Changed, Create Channel, Move Member, Delete Channel |
| Utility commands | User Info, Member Info, Server Info, Bot Info, Mention, Timestamp |

## Events

| Block | Status | Outputs | Notes |
|---|---|---|---|
| Bot Started | ✅ | started at | |
| On Error | ✅ | error, block | outside Try Zones |
| On Signal | ✅ | data | from Emit Signal |
| Slash Command | ✅ | interaction, user, member, channel, server, options | |
| Button Pressed | ✅ | interaction, custom id, message, user | |
| Message Sent | ✅ | message, author, channel, content | MessageContent intent |
| Member Joins / Leaves | ✅ | member, server | GuildMembers intent |
| Reaction Added | ✅ | reaction, message, user, emoji | |
| Select Menu Used | ✅ | interaction, custom id, values (list), user | string, user, role, channel menus |
| Modal Submitted | ✅ | interaction, custom id, fields (object), user | answers by input id |
| Autocomplete | ✅ | interaction, option, typed text | answered with Respond to Autocomplete |
| Context Menu Used | ✅ | interaction, target user or message | needs context menus in the Slash Commands editor |
| Message Edited | ✅ | before, after, author | before only when cached |
| Message Deleted | ✅ | message (partial), channel | also bulk deletes |
| Reaction Removed | ✅ | reaction, message, user, emoji | |
| Member Updated | ✅ | before, after, roles added, roles removed, nickname, timed out, boosting | GuildMembers |
| Member Banned / Unbanned | ✅ | user, server, reason | GuildModeration |
| Every / At Time | ✅ | time | interval or daily/weekly time; core block |
| Voice State Changed | ✅ | member, before channel, after channel, joined/left/moved | GuildVoiceStates |
| Channel Created / Deleted / Updated | ✅ | channel, before | |
| Role Created / Deleted / Updated | ✅ | role, before | |
| Thread Created | ✅ | thread, owner | |
| Bot Added to / Removed from Server | ✅ | server | |
| Invite Created / Deleted | ✅ | invite | GuildInvites |
| Poll Vote | ✅ | poll, answer, user | GuildMessagePolls |
| Presence Changed | ✅ | member, status, activity | privileged GuildPresences |
| AutoMod Action | ✅ | rule, action, member, content | |
| Scheduled Event Started / User Interested | ✅ | event, user | |
| Web Request Received | ✅ | method, path, body | for dashboards and webhooks from other services |

## Messages

| Block | Status | Fields → Outputs | Notes |
|---|---|---|---|
| Send Message | ✅ | mode, format, text, embeds, components, private → message | reply, edit, follow-up, defer, send, reply to message, edit message; format Layout sends a Build Layout message |
| Build Layout (Components V2) | ✅ | layout designer → layout | containers, text, sections, media galleries, files, separators, buttons and select menus |
| Build Embed | ✅ | embed designer → embed | |
| Button | ✅ | label, style, custom id, url, emoji → component | |
| Add Reaction | ✅ | message, emoji | |
| Delete Message | ✅ | message, delay | |
| Message Info | ✅ | message, info → value | |
| Send Message: Update message | ✅ | new mode for button and menu presses | edits the message the button is on |
| Send Message: Direct message | ✅ | new mode with a user input | |
| Send Message: files | ✅ | attachments (URL or text as file) | |
| Select Menu | ✅ | custom id, placeholder, options or user/role/channel type, min/max → component | |
| Show Modal | ✅ | interaction, custom id, title, text inputs | answer is Modal Submitted |
| Text Input | ✅ | id, label, short/paragraph, required, min/max, value → component | for Show Modal |
| Respond to Autocomplete | ✅ | interaction, choices (list) | up to 25 |
| Wait for Reply | ✅ | channel or message, from user, timeout → message or timed out | message, button or reaction collector |
| Bulk Delete | ✅ | channel, count, only from user → deleted count | messages under 14 days |
| Get Messages | ✅ | channel, count, before/after → list | history, transcripts |
| Find Message | ✅ | channel, id → message | |
| Pin / Unpin Message | ✅ | message | |
| Remove Reaction | ✅ | message, emoji, user (or all) | |
| Get Reactions | ✅ | message, emoji → users, count | starboards, giveaways |
| Create Poll | ✅ | channel, question, answers, duration, multi → message | |
| End Poll / Poll Results | ✅ | message → answers with counts | |
| Send via Webhook | ✅ | channel or webhook URL, name, avatar, text, embeds | |
| Publish Message | ✅ | message | announcement channels |

## Members and users

| Block | Status | Notes |
|---|---|---|
| Find Member | ✅ | by id |
| Member Info / User Info | ✅ | |
| Has Role | ✅ | |
| Add Role | ✅ | |
| Moderate Member | ✅ | time out, kick, ban, unban, remove role, nickname |
| Check Permission | ✅ | member (or the bot) has a permission, optionally in a channel → yes/no |
| List Members | ✅ | server, has role, limit → list (GuildMembers) |
| Voice: Move / Disconnect / Mute / Deafen | ✅ | member, channel |
| Warnings | ✅ | add, list, clear warnings kept in stored data (a moderation helper, or a template) |

## Roles

| Block | Status | Notes |
|---|---|---|
| Find Role | ✅ | by id or name → role |
| Create Role | ✅ | name, color, hoist, mentionable, permissions → role |
| Edit Role | ✅ | role, the same fields |
| Delete Role | ✅ | |
| List Roles | ✅ | server → list |
| Role Info | ✅ | members count, color, position, permissions |

## Channels, threads and the server

| Block | Status | Notes |
|---|---|---|
| Find Channel | ✅ | by id or name |
| Create Channel | ✅ | type (text, voice, category, announcement, forum, stage), name, category, topic → channel |
| Edit Channel | ✅ | name, topic, slowmode, NSFW, category, position |
| Delete Channel | ✅ | |
| Set Channel Permissions | ✅ | channel, role or member, allow/deny permissions |
| Lock / Unlock Channel | ✅ | shortcut for denying Send Messages to everyone |
| Create Thread | ✅ | from message or in channel, name, private, auto-archive → thread |
| Thread actions | ✅ | archive, lock, add or remove member |
| Create Forum Post | ✅ | forum, title, text, tags |
| Server Info | ✅ | member count, owner, created, boosts, icon |
| Create Invite | ✅ | channel, max uses, max age → invite URL |
| Audit Log | ✅ | server, action type, limit → entries (who did it) |
| Scheduled Events | ✅ | create, edit, list |
| Emojis and stickers | ✅ | create, delete, list |
| AutoMod Rules | ✅ | create, list |
| Edit Server | ✅ | name, icon, system channel |

## Bot

| Block | Status | Notes |
|---|---|---|
| Set Bot Status | ✅ | status, activity |
| Bot Info | ✅ | ping, uptime, server count, user |
| Set Bot Name / Avatar | ✅ | rate limited by Discord |

## Formatting (values)

| Block | Status | Notes |
|---|---|---|
| Mention | ✅ | user, member, role or channel → `<@id>` text |
| Timestamp | ✅ | date, style (relative, short, long) → `<t:…:R>` text |
| Discord Text | ✅ | bold, italic, code, code block, spoiler, quote, escape markdown |
| Emoji | ✅ | custom emoji by name → text |
| Format Number | ✅ | thousands separators, decimals, currency, ordinal (1st) — core |
| Duration | ✅ | "10m" → milliseconds, milliseconds → "2 h 5 min" — core |

## Data, flow and logic (core)

| Block | Status | Notes |
|---|---|---|
| Get / Set / Increase Stored Value | ✅ | scopes: bot, server, user, member, channel |
| Get / Set Variable | ✅ | |
| If, Switch, Compare, And/Or/Not | ✅ | |
| For Each, Repeat, Wait | ✅ | |
| Emit Signal / On Signal | ✅ | |
| Try Zone, Throw Error | ✅ | |
| Text, Join Text, Text Tools | ✅ | |
| Math, Random Number | ✅ | |
| Make List, List Tools, Get Property, JSON, Date & Time | ✅ | |
| Web Request | ✅ | |
| Cooldown | ✅ | key (user, member, channel or text), seconds → ready or wait, seconds left |
| Delete Stored Value | ✅ | |
| Leaderboard | ✅ | scope, key, top N, server → ranked list with user and value |
| List Stored Keys | ✅ | scope, prefix → list |
| Call Workspace | ✅ | runs another workspace like a function, with inputs and a return value |
| Loop While | ✅ | with a safety limit |
| Make Object / Set Property | ✅ | build objects for JSON and web requests |
| Regex Match | ✅ | pattern → matches, groups |

## Editor and runtime

- ✅ **Slash Commands editor:** subcommands and groups (names with spaces: `ticket open`),
  who can use a command, DMs, autocomplete options, a Discord preview. Context menu commands
  come from their Context Menu Used blocks.
- ✅ **Custom id routing:** button, menu and modal events match a prefix (`vote:`) or a pattern
  and give the rest of the id.
- ✅ **Intents** are collected from every block in the project.
- ✅ **Templates:** File › New Project starts from Welcome & auto role, Moderation, Levels & XP,
  Tickets or Role menu.
- ✅ **JavaScript in texts:** every text field takes `${…}` with any JavaScript (`${#4.message.url}`, `${xp * 2}`); names nothing earlier gives get an input.
- ✅ **JSON views:** the Embed Builder, Layout Builder and Slash Commands switch between the builder and Discord's JSON, and take pasted JSON.
- ✅ **Slash command settings:** translations, choice values, length limits, channel kinds, several permissions, contexts, install types, age-restricted.

## Not built yet

Everything planned here is built. Since this list was written: dedicated pin types for threads,
voice channels, invites, polls and webhooks; privileged intents in Bot settings; errors that
name the missing permission; creating stickers; and Components V2 in forms (Form Field, File
Upload and Form Text, and select menus in Show Modal).
