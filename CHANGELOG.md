# Changelog

All notable changes to XC™ Circuit. Versions follow [semantic versioning](https://semver.org); while in Early Access (0.x), a new minor version may change how things work. Projects are upgraded automatically when they open.

## Unreleased

### Added
- **Select several workspaces and delete them together**: Ctrl-click workspaces in the Workspaces list to select several, or Shift-click to select all from the last one clicked, then press Delete. It asks first, listing them. Clicking elsewhere (or Escape) lets go of the selection, so Delete on the canvas still deletes blocks.

### Fixed
- **Comparing Discord ids is exact**: Check, If and the other comparisons compared long ids (18-19 digits) as numbers, which can't hold that many digits exactly, so two different ids that differ only in their last digits could count as equal. Whole numbers that long are now compared exactly.
- **Send Message shows Components again**: since advanced pins start hidden, new Send Message blocks had no Components input for buttons and select menus. It's a normal input again. On a block made in between, click the small dot next to Components to make it a pin.
- **Modal Builder: Cancel and Submit keep their size**: with a short form, the preview's Submit button stretched to fill the space below it.

## 0.1.8

### Added
- **A Modal Builder for forms**: Show Modal's form is now designed in one place, like a layout: a title, and up to 5 questions (a box to type into, a select menu or a file upload, each with a hint) and texts, with a preview that looks like Discord's. **Edit form…** on the block opens it; the answer ids are listed there to copy into Modal Submitted. The separate Text Input, Form Field, File Upload and Form Text blocks are no longer needed and left the block picker; forms built from them keep working.
- **Invite your bot**: Bot settings › Connection now has an invite link under the bot token once one is set. Pick the permissions (common bot permissions, Administrator or none), then open Discord's page for adding the bot to a server, or copy the link to share it.

### Changed
- **Missing required inputs glow red**: an input a block needs that is still empty or unconnected now has a soft red glow that pulses slowly, and its name turns red, so it's easy to spot on the canvas.
- **Problem counts show a warning sign**: the number of problems next to a workspace (or a closed folder) in the Workspaces list now has a warning triangle in front of it.
- **Create Channel: Position is on the block**: the channel's place in the list (0 at the top, -1 at the end) is now a normal input you can type or connect, instead of an advanced setting.
- **Pins of advanced settings start hidden**: a new block shows only its usual pins; an advanced one (such as Insert Row's More values) appears once turned on with its pin button in the Inspector.

### Fixed
- **Updating from inside the app works while an AI client is connected**: Setup couldn't replace XC Circuit while an AI client (Claude, Cursor …) had its MCP server running, so it undid the update. It now moves the running copy aside and finishes. Setup also keeps a log of the update now, `setup.log` in the `xc-circuit-update` folder in your temporary files.
- **Older graphics cards are used again instead of software drawing**: cards too old for DirectX 12 and Vulkan, such as a Radeon HD 5000, got Windows' slow software renderer. When the app finds itself drawing in software, it now checks whether OpenGL can use the card and, if so, starts again with OpenGL, and keeps using it from then on.
- **AI agents can't save slash commands Discord would refuse**: `set_slash_commands` now checks names the way the Slash Commands editor does (1–32 lowercase letters, numbers, - or _ per word; a space starts a subcommand) and says what's wrong instead of saving them.

## 0.1.7

### Added
- **An Updates window**: Help › Check for Updates now opens a window that shows whether a newer app is out and, for each bundled plugin, your version and the published one (up to date, newer, or needing a newer app). **Update blocks** gets the newer ones; **Download blocks again** downloads the published blocks again even when you have them, for when blocks seem out of date or broken. When GitHub can't be asked, the window and the Console say why, instead of "up to date (or GitHub couldn't be asked)".

### Changed
- **Get Property: pick the property from a list, or connect it**: with a value connected whose properties are known (a Discord user, member, channel, message … or an object of a known shape), Property lists them; **Custom…** at the end shows a text box for any other, such as `guild.name`. Property can also be connected now, to choose it while the bot runs. So can the variable name of Get Variable and Set Variable.
- **Create Transcript installs sharp**, which better-discord-transcripts asks for: pictures kept in a transcript are made smaller, and the Console no longer warns that it's missing.
- **Errors show in full in the Console**: a block's error now comes with the rest of what the bot knows about it, each on its own line: the original message (when a clearer one is shown), the kind of error with its code, HTTP status and request, where in the code it happened, and what caused it.

### Fixed
- **Block updates arrive right after they're published**: the app asked for the list of published blocks in a way GitHub could answer with a copy from a few minutes before, so a check right after an update could still say "up to date".
- **A channel id typed into Send Message works**: with "Send to channel", a channel id in the Channel box gave "target.send is not a function", because the bot didn't look up ids for inputs a block adds itself (Send Message adds Channel only for that mode). It does now, on Discord and Fluxer, and Discord's Send Message looks the id up itself too, so XC Circuit 0.1.6 gets the fix with the Discord Bot 1.2.5 block update.
- **Send Message takes files again**: its Files input was under Advanced, and inputs there had no pin on the block, so nothing could be connected to it. Files now shows on the block (Discord and Fluxer; not when deferring, or when editing on Fluxer), ready for a picture from Build Canvas, a file path from Read, Write or Download File, a web address or another message's attachments.
- **Advanced inputs can be connected**: in the Inspector's Advanced part, inputs that can only be connected (such as Components) have the **pin** chip, which puts their pin on the block; inputs that are pins or connected always show on the block.

## 0.1.6

### Added
- **Channel inputs take a channel id**: every channel input (categories, threads and voice channels too, on Discord and Fluxer) can be switched from its pin to a box for the channel's id, with the pin toggle on the block or in the Inspector. A text wired in works as well, also as a mention such as `<#123…>`; the bot finds the channel before the block runs, and says so when no channel has that id. Plugins can offer this for their own types with `fromId` on a pin type.
- **Permissions are ticked, not typed**: Set Channel Permissions (Allow and Deny), Create Role and Edit Role on Discord, and Set Channel Permissions on Fluxer pick permissions from a list with checkboxes and a search. What older projects typed still works and shows ticked. Plugins can use it too: the new `multiselect` field type.
- **Value blocks can go in the flow**: Get Property, Math, Compare, Text and the other value blocks have Action pins now. Wired into a flow, they run right there, once, and the blocks after them get that result, so it's plain when they run. Left to the side, they work as before: worked out when a block needs them.
- **Events say which server they're in**: Button, Select Menu, Modal, Autocomplete, Context Menu, Message (received, edited, deleted), Reactions, Poll Vote, Presence and Thread Created now have a **Server** output (empty in DMs); Autocomplete and Context Menu also got **Member** and **Channel**, Modal, Reactions and Poll Vote **Channel**. On Fluxer, events that only gave a server id now give the server too, and Reactions and Thread events its id.
- **The Console has a text filter**: type in the Filter box to see only lines with that text, together with a level chip if you like.
- **Slash commands in the side bar can be ordered, put in folders and turned off**: drag a command above or below another or into a folder, make folders with the folder button (rename by double-click, drag them into order), and right-click › Turn off to keep a command from being registered with Discord. Clicking a command opens Slash Commands on it.
- **AI agents can work on your bot (MCP)**: XC Circuit is now an MCP server, so AI agents such as Claude can read your workspaces, add, change and connect blocks, look up block types, see problems and the Console, edit slash commands, and start or stop the bot. Every change shows in the editor at once and can be undone; the bot's token is never given out. It comes in the new **AI agents (MCP)** plugin: **Plugins › AI agents panel** adds it to Claude Desktop, Claude Code, Cursor or VS Code with one click, or copies the settings for any other client. A `run_script` tool lets an agent do many things in one step (build a whole flow with JavaScript that calls the other tools). Every tool an agent uses is written to the Console as an **MCP** line, with what it was given (and why, when it failed); the MCP chip shows only those. Agents can see the canvas with a `screenshot` tool and know how big blocks are, so new blocks go to a free spot instead of on top of others. They can also add and change notes, fill embeds, layouts and canvases (in the builders' format or Discord's own JSON) and open a builder to show you. AI clients start `xc-circuit --mcp`, which talks to the open app on this computer only.
- **Plugins can offer AI agents tools** with `xc.tool({ name, description, input, run })`, and read more of the editor: `xc.blockTypes`, `xc.blockType`, `xc.blockPins`, `xc.connect` (checked like a drawn wire), `xc.addWorkspace`, `xc.problems`, `xc.console`, `xc.t` and `xc.copyText`.
- **Optional plugins that come with the app**: a bundled plugin with `optional = true` in its plugin.toml (the AI agents one) can be turned off and removed like your own; **Add back** at the end of the Plugins list brings a removed one back.

### Changed
- **Embed and layout previews on blocks are the same size**: the embed preview was drawn smaller (86%) than the layout preview; both are now drawn as in the builders, with Discord's own sizes (embed text a little smaller than message text, as in Discord).
- **The buttons above a selected block** wrap into two rows when the block is narrow, stay inside the canvas, and no longer have Source: the block's source file opens from the Inspector.
- **Slash commands are checked against Discord at every start**: the bot reads the commands Discord has and registers them again when they differ from the project's, so commands changed or removed elsewhere (in the developer portal, by another copy of the bot, or with a lost data folder) are put right. Before, it only compared with what it had sent last time.

## 0.1.5

### Changed
- **Embed previews are as wide as Discord makes them**: an embed is as wide as its content needs (a short "Pong!" makes a small card), up to Discord's widest (516 points), on blocks and in the Embed Builder, instead of always filling the room. Layout previews on blocks stop at the width the Layout Builder shows them at, however wide the block is made.
- **Build Layout shows the message on the block**: instead of a list of its parts, the block shows the layout as Discord does (like the Embed Builder's blocks), up to about 340 points tall; longer ones fade out at the bottom. Blocks with a layout start wider to make room for it.
- **A placeholder where the workspaces go**: closing the last workspace tab (or deleting its workspace) leaves a "No workspace" tab in its place, listing the project's workspaces to open and a button for a new one, instead of the Inspector taking the whole window. Opening a workspace puts it back right there.
- **The builders fit narrow tabs**: when there isn't room for the preview beside the form, the Embed and Layout Builders show it on top, with the values from earlier blocks at the end of the form. Slash Commands shows its Discord preview above the command's settings and starts with just the list of commands; once you pick one, the list folds into a slim bar on the left (with how many commands there are, and a dot when one needs fixing) that opens it over the form with a click. And the Canvas Builder puts the picture on top with its settings in two columns below.

### Fixed
- **Marked text shows in the text boxes on blocks**: when the canvas wasn't at 100%, selecting text in a block's text box didn't highlight it. It does now, and marked text everywhere is highlighted more strongly.
- **Fluxer's Bot Info gives a real ping**: fluxer.js doesn't measure one, so Ping was always 0; Bot Info now times a request to Fluxer.
- **Bot Info's Ping is never -1**: in the first 40 seconds or so after the bot starts, discord.js doesn't know the ping yet and said -1; Bot Info now times a request to Discord instead until it does.
- **The text cursor shows in text boxes**: the app now asks Windows whether its window is in front, instead of waiting for a focus message that could be missed while the window was still hidden at startup (without it, no cursor is drawn).
- **Blocks grow to fit their dropdowns**: a choice too long for the block (the Captcha block's "To the interaction (only they see it)", say) made the dropdown stick out over the labels; the block now gets wide enough for it.
- **New Project remembers the folder you browse to**, right when you choose it, not only once a project is made there. And it tells you when the folder isn't there (a drive that's not connected, say) or is a file, before you press Create.
- **Long starter names in New Project stay on one line**: a name too long for its card (in German, say) ends in … instead of running into the description below it; hover the card for the whole name.
- **The builders' windows scroll properly again**: the Embed, Layout and Canvas Builders and Slash Commands no longer show scrollbars all the time, and their Cancel and Apply buttons stay at the bottom where you can reach them, with the wheel scrolling what's above. A builder's window can't be made narrower than the builder needs; docked into a narrow area, it scrolls sideways only.

## 0.1.4

### Changed
- **Canvas text stays readable when zoomed out**: below about 9 points on screen, labels, titles and values are drawn bold instead of in the light font, grey text gets brighter, and the smallest text gets a little bigger (never more than 15% wider than its place, so it doesn't run into its neighbours). Text that wraps in a box (a message, the embed preview) keeps its size and only gets brighter. At 100% and closer nothing changes. Zooming is a little smoother too: text laid out for one zoom is reused for the next.

## 0.1.3

### Changed
- **The Workspaces list shows more**: each workspace's problems, a dot while its blocks run (red after one failed), and a mark when it's turned off; folders show how many workspaces they hold. Hover for its blocks, events and when it last ran. The menus have icons, and **Duplicate** makes a copy of a workspace (turned off, so it doesn't answer twice).
- **Menus switch on hover**, as in Windows: with File (or any menu) open, pointing at another menu's name opens it and closes the first.
- **Workspaces can be put in order**: drag one above or below another (a line shows where), into a folder, or drag folders to order them. The order is saved with the project.
- **The activity bar's icons work like tabs**: drag one to put them in another order, or out onto the main area, where it becomes a tab (next to the tabs under the pointer, or in a new area near an edge). Drag a tab (Workspaces, Stored Data, Plugins, the Inspector, Console, Problems or a plugin's panel) onto the activity bar and it becomes an icon there, shown in the side bar. Right-click an icon (or the side bar's heading) › Move to the main area. Closing Workspaces', Stored Data's or Plugins' tab puts it back in the bar; View › Reset layout puts the bar back too. View › Side bar and View › Panels tick what's open, as an icon or a tab: untick to take it away, tick to bring it back (Workspaces, Stored Data and Plugins into the activity bar, panels as a tab).
- **The tabs and panels are XC Circuit's own** (no longer egui_dock). They work as before: drag a tab next to other tabs, onto a side of an area to split it, or out of the main area into a window of its own. While dragging, a line or a lit area shows where it goes, and the tab follows the pointer like the activity bar's icons. Right-click a tab › Close or Move to the activity bar; middle-click closes it. A window moves by the empty part of its tab bar. The separators between areas can be grabbed over a canvas too. The layout you had is kept.
- **Built on egui 0.37** (from 0.36), now that the editor's own tabs no longer wait for egui_dock to catch up.
- **The builders keep a size they work at**: the Embed, Layout and Canvas Builders and Slash Commands docked into a small area (or a small window) scroll both ways instead of squeezing their columns.
- **The Console wraps long lines** to its width; narrow, the time and level get a row of their own.
- **Tabs have icons** before their names: workspaces, Inspector, Console, Problems, the builders, Settings …
- **The tour shows you where to click**: whatever comes next glows (the Add block button, the Math entry in the picker, the two pins to wire with a hand that shows the drag, the Inspector's fields, Run), the card says what to do right now and notices mistakes, such as a wire to the wrong pin or a wrong number.
- **New project shows templates as smaller cards, grouped** into Basics, Community, Moderation, Notifications and Games & servers.
- **Tickets save a transcript** when they close and send it to the member in a DM.
- **Labels sit in the middle of their rows** next to taller lists, sliders and fields: in the Canvas Builder, Bot › Settings and Settings. Rows of buttons that don't fit go on to the next line (the Plugins side bar's did not).
- **Bot › Settings looks like the app's Settings**: pages on the left with icons (Connection, Slash commands, Messages, Music, Privileged intents), the page on the right, Save at the bottom. The list shows which page still needs a required setting or has a value to fix.

### Added
- **A Plugin guide** on the website: how to make a plugin (what goes where, the save-and-reload loop, where errors show, TypeScript), recipes to copy (value blocks, branching, web APIs, npm packages, commands, checks, panels, settings, bot events), and how to share and update one (versions, zips, changing blocks people use).
- **The Console's font, size and line spacing** (Settings › Editor › Console): any font installed on the computer, monospaced ones first, with a search and a preview; 8 to 32 pt; 0 to 16 pt between lines. Characters the font lacks come from the built-in one. Its lines are as tall as the text now (they were as tall as a button), and *Show block* is a link in the line.
- **Stored Data reads and changes everything the bot keeps**: add, change and delete stored values (in `store.json` or the database they're kept in), and delete a whole bucket from its right-click menu. Each of the bot's databases shows its tables as a grid, as database tools do: a page of rows, a key on the primary key's columns, NULL as NULL, columns to resize, and Change and Delete in front of each row (Delete asks once more). Double-click a cell to change it right there; **Add row** adds one. SQLite, MariaDB/MySQL, PostgreSQL and MongoDB, reached the way the bot reaches them. Stored values are read-only while the bot runs (it reads them when it starts); tables can be changed any time. Each database stays connected while you work in it (for ten minutes after the last request), so showing and changing things is quick.
- **Plugins can be turned off and removed** in the Plugins view of the side bar: a switch turns a plugin off (it's still listed, to turn it on again), the trash can removes it (it asks first, and says which folder goes). The plugins that come with the app can't be turned off or removed.
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
- **Screen readers can name every control**: text boxes, dropdowns, numbers, sliders, switches and colours take the name of the setting, field or caption they belong to; icon buttons say what they do; tabs, the activity bar, the title bar's buttons, the side bar's tree, blocks and their pins on the canvas, the pickers' rows and the colour picker say what they are and what they're called. A test walks the main screens (First Start, the editor, every Settings and Bot › Settings page, each builder, New project) with egui's accessibility check, so none is left without a name.

### Removed
- **Native (compiled Rust) plugins**, and Project Stats, the example one. Plugins change the editor with JavaScript, which does commands, panels and checks in a sandbox; native ones had full access to your computer and had to be built with exactly the app's own compiler and libraries. Settings › Plugins no longer has *Allow native plugins*, and the Plugins tab is gone: the Plugins view of the side bar lists what's installed and how it runs (a plugin with only a native part says so; its blocks and settings still load).

### Fixed
- **A project inside a folder of JavaScript modules** (one whose `package.json` says `"type": "module"`) can run its bot, and Test connection and Stored Data reach its databases: Node read the runtime as a module and stopped.
- **The unsaved mark in the window's title and the Inspector's pin switch** showed an empty box: the fonts have no "●". A test now checks that every text of the interface, English and German, can be drawn.

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
