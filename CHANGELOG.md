# Changelog

All notable changes to XC™ Circuit. Versions follow [semantic versioning](https://semver.org); while in Early Access (0.x), a new minor version may change how things work. Projects are upgraded automatically when they open.

## Unreleased

### Changed
- **The Workspaces list shows more**: each workspace's problems, a dot while its blocks run (red after one failed), and a mark when it's turned off; folders show how many workspaces they hold. Hover for its blocks, events and when it last ran. The menus have icons, and **Duplicate** makes a copy of a workspace (turned off, so it doesn't answer twice).
- **Menus switch on hover**, as in Windows: with File (or any menu) open, pointing at another menu's name opens it and closes the first.
- **Workspaces can be put in order**: drag one above or below another (a line shows where), into a folder, or drag folders to order them. The order is saved with the project.
- **The activity bar's icons work like tabs**: drag one to put them in another order, or out onto the main area, where it becomes a tab (next to the tabs under the pointer, or in a new area near an edge). Drag a tab (Workspaces, Stored Data, Plugins, the Inspector, Console, Problems or a plugin's panel) onto the activity bar and it becomes an icon there, shown in the side bar. Right-click an icon (or the side bar's heading) › Move to the main area. Closing Workspaces', Stored Data's or Plugins' tab puts it back in the bar; View › Reset layout puts the bar back too.
- **The tabs and panels are XC Circuit's own** (no longer egui_dock). They work as before: drag a tab next to other tabs, onto a side of an area to split it, or out of the main area into a window of its own. While dragging, a line or a lit area shows where it goes, and the tab follows the pointer like the activity bar's icons. Right-click a tab › Close or Move to the activity bar; middle-click closes it. A window moves by the empty part of its tab bar. The separators between areas can be grabbed over a canvas too. The layout you had is kept.
- **Built on egui 0.37** (from 0.36), now that the editor's own tabs no longer wait for egui_dock to catch up. Native plugins are built against it too: rebuild yours with the workspace.
- **The Console wraps long lines** to its width; narrow, the time and level get a row of their own.
- **Tabs have icons** before their names: workspaces, Inspector, Console, Problems, the builders, Settings …
- **The tour shows you where to click**: whatever comes next glows (the Add block button, the Math entry in the picker, the two pins to wire with a hand that shows the drag, the Inspector's fields, Run), the card says what to do right now and notices mistakes, such as a wire to the wrong pin or a wrong number.
- **New project shows templates as smaller cards, grouped** into Basics, Community, Moderation, Notifications and Games & servers.
- **Tickets save a transcript** when they close and send it to the member in a DM.
- **Labels sit in the middle of their rows** next to taller lists, sliders and fields: in the Canvas Builder, Bot › Settings and Settings. Rows of buttons that don't fit go on to the next line (the Plugins side bar's did not).
- **Bot › Settings looks like the app's Settings**: pages on the left with icons (Connection, Slash commands, Messages, Music, Privileged intents), the page on the right, Save at the bottom. The list shows which page still needs a required setting or has a value to fix.

### Added
- **Transcripts**: **Create Transcript** saves a channel's messages as an HTML file that looks like Discord (embeds, Components V2, replies, threads, polls), for tickets: the latest N, without bots if you like, pictures kept if you like. Drawn completely on the bot, so the file loads no scripts and names or messages can't put code into it.
- **Captcha** (Discord and Fluxer): checks that someone is a person, then Passed, Failed or Timed Out. On Discord in a message only they see (or a DM): type the code from a picture, solve a sum, click the named emoji, or just click; on Fluxer the code or a sum, answered in a channel.
- **Server tools** (a new bundled plugin): **SSH Command** (`${…}` values quoted for the shell; the server's key is remembered and checked), **Server Usage** (CPU, memory, disk, uptime; here or over SSH), **Docker** (list, start, stop, restart, logs), **RCON Command** (Minecraft, Rust, ARK …), **Minecraft Server** (Java and Bedrock status) and **Port Check**. SSH, RCON and Docker only run once allowed in Bot › Settings › Server tools.
- **Files**: **Read File** and **Write File** (text, JSON, CSV tables, lines or bytes; replace or add to the end), **List Files**, **File Action** (check, delete, copy, move, make a folder) and **Download File** (from the web or an attachment), all in the bot's own files folder.
- **New-post alerts**: the new **On New Post** event runs for each new YouTube video (Shorts optional), Reddit post, Bluesky or Mastodon post, GitHub release, Steam news item or RSS/Atom feed entry, and when a Twitch or Kick channel goes live (with keys in Bot › Settings › New-post alerts). It remembers what it announced, also over restarts, and starts without announcing old posts.
- **Timers that run once**: **Schedule** (after 10m, 3d … or at a time, with a value along), **On Scheduled**, **Cancel Scheduled** and **List Scheduled**, for reminders, temporary bans or giveaways. They're kept with the stored data, so they survive restarts; ones missed while the bot was off run when it starts. A key gives one timer per person.
- **Pictures**: the new **Build Canvas** block draws welcome cards, rank cards and other pictures, designed in the new **Canvas Builder**: shapes, circles, pictures (such as avatars, round), texts and progress bars, with gradients, outlines and shadows, dragged into place on a live preview. Start from a Rank, Welcome or Profile card. Send the picture with Send Message's Files. The bot draws with the same fonts the preview uses, also in exported bots.
- **Fluxer: Send Message sends files** (under Advanced): a picture from Build Canvas, web addresses, file paths or another message's attachments.
- **Databases**: connect MariaDB, MySQL, PostgreSQL, SQLite or MongoDB in Bot › Settings › Databases (with Test connection), and use them with the new blocks **Get Rows**, **Insert Row**, **Update Rows**, **Delete Rows** and **SQL Query**. Each block picks a database or the default one. Values are always sent separately from the SQL, so what people type can't change a query. Passwords stay in the system's password store.
- **Stored values can live in a database** instead of store.json: Bot › Settings › Databases › Keep stored values in. The first time, store.json's values are copied over.
- **New templates** for Discord and Fluxer: Verify with a captcha, Reminders, New-post alerts and Minecraft server status.
- **Servers and databases given on the block**: SSH Command, Server Usage, RCON Command, Docker and the database blocks can connect to a server the block is given, such as one from a slash command's options, instead of the one in Bot › Settings. The password can only be connected, never typed into the block, so it isn't saved in the project. Each kind has its own switch in Bot › Settings, off until you allow it.
- **One UI kit for the whole app**: every page, builder, dialog and menu now draws its buttons, fields, numbers, sliders, checkboxes, switches, dropdowns, colours, notes and rows from the same kit, at one height and in one style. Checkboxes and radio choices have the app's look instead of egui's, numbers are as tall as the fields beside them, sliders show their number to type.
- **Plugins use the same kit**: Bot › Settings pages can hold notes (info, warning, success), links, multi-line texts, colours, sliders, lists of texts and files or folders with Browse… (the DemoBot example has a page with them all), and editor plugins' panels get `ui.toggle`, `ui.select`, `ui.number`, `ui.slider`, `ui.color` and `ui.note`; their buttons, check boxes and text boxes now look like the app's.
- **Plugins can add pages to Bot › Settings** (`contributes.botSettings`), with switches, choice lists and numbers besides texts and secrets. Settings reach the bot with their default until you change them.

### Security
- **Addresses people type stay on the public web**: HTTP Request and Download File only reach this computer or its network for an address you typed yourself; one filled in from `${…}` or a wire must be public, redirects too (*Private addresses too* under Advanced allows it). HTTP Request reads at most 10 MB of an answer, Download File stops at its limit also when the server doesn't say how big the file is.
- **Build Canvas pictures filled in from `${…}`** are a web address (a public one) or a file in the bot's files folder, never a path anywhere on the computer.
- **Music plays no files from the bot's computer**: Play Music and Search Music refuse paths (a file in the bot's files folder, by its name, still works).
- **Bots install axios 1.x** where the Deezer music source asked for 0.27, which has known security holes.
- **The editor no longer builds in `paste`**, which is no longer maintained (RUSTSEC-2024-0436): it came with egui_dock, which the editor's own tabs replace.

## 0.1.2

### Added
- **Updates install themselves** (from the Windows Setup, or an AppImage): Update and restart downloads the new version, checks it, installs it as XC Circuit closes and opens it again. Other kinds still open the release page.

### Fixed
- The update window shows what's new formatted (headings, lists, bold), not as raw Markdown.

## 0.1.1

### Fixed
- Text boxes with a button after them (Stored Data's filter, New Project's location) leave the button the room its text needs in every language, and are as tall as the button.
- The interface language in Settings no longer moves around so you can't click it; it's a dropdown now, with room for any number of languages.

## 0.1.0 – Early Access

The first public release.

### Building bots
- A canvas editor for **Discord** (discord.js) and **Fluxer** (fluxer.js) bots: events start a flow, actions run in order along green wires, values are worked out when an action needs them.
- **242 blocks**: messages, embeds and Components V2 layouts, slash commands with options and subcommands, buttons, select menus and modals, moderation, roles, channels, members, voice and music, stored data, variables, timers, web requests, JSON, text, numbers, lists, dates, and Run Code / Use Method for anything else.
- **JavaScript in every text** with `${…}`, with completion for the values earlier blocks give and their members.
- **Try Zones** that retry, time out and catch errors; an On Error event.
- **Forms** with text boxes, menus, file uploads and text (Components V2), and Modal Submitted gives every answer.
- **Fluxer bots** with 62 blocks: roles, bans, emojis and stickers, webhooks, invites, threads, channel permissions, reactions, Wait For, the audit log, and events for bans, roles, channels, threads, members, voice, typing and more.
- **Clear errors** when the bot is missing a permission: they say which one, and where to give it. Bot settings lists the privileged intents your blocks need, with a link to turn them on.
- **Workspaces** in folders, which can be turned off to leave them out of the bot without deleting them.
- **Notes** to explain and group blocks: title, description, colour, move (with the blocks inside, or alone), resize, copy.
- **Embed, Layout and Slash Command Builders** with live Discord previews and Discord's own JSON.
- **Snap to grid**, **align and spread** selected blocks, **favourite and recent** blocks in the picker.
- **Run with one click** (F5): blocks glow as they run, show how long they took, and failures show on the block. **Hover a wire** to see the last value that went through it.
- **Export** a bot as a folder that runs with `node index.js` anywhere.
- **Starter bots** in New Project: welcome & auto role, moderation, levels & XP, tickets, role menu; basic commands and moderation for Fluxer. **DemoBot** example.

### Plugins
- Blocks are plain **JavaScript or TypeScript** files that reload when saved.
- **Editor plugins** in JavaScript: panels, commands, checks, overlays (a desk pet!), timers, sounds, pictures, placing and selecting blocks. Sandboxed, no approval needed.
- **Native plugins** in Rust with egui, off until allowed.
- Build modes, pin types, themes, translations and starter bots come from plugins.
- **Install plugins from a zip** (Plugins › Install from Zip…, or drop the file on the window).

### Comfort
- Command palette, changeable **keyboard shortcuts**, themes and accent colours, touch mode, interface size, English and German.
- Backups on every save with restore, crash recovery for unsaved changes, and **crash logs with a ready bug report**.
- Update check (Help › Check for Updates), and **block updates** without an app update: newer versions of the bundled blocks download and load right away (Help › Undo Block Updates goes back).
- Installers: Windows Setup (x64 and ARM64), Linux AppImage, .deb, .rpm and .tar.gz.
- A [website](https://xcrafttm.github.io/XC-Circuit/) with tutorials, explanations and the full plugin API.
