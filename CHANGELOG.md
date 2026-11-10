# Changelog

All notable changes to XC™ Circuit. Versions follow [semantic versioning](https://semver.org); while in Early Access (0.x), a new minor version may change how things work. Projects are upgraded automatically when they open.

## Unreleased

### Fixed
- **Stored values named like `constructor` or `toString` work like any other**: every JavaScript object answers to a few names by itself. Get Stored Value gave a function instead of the fallback for a name such as `constructor` that was never stored, and Has Stored Value said yes. A value stored as `__proto__` wasn't kept at all, and what was in it then showed up under the bucket's other names. That mattered where the name comes from what people type (a tag command, say). Only names that were really stored count now.

## 0.2.10

### Changed
- **Saving writes only the files that changed**: every save wrote every workspace's file again, also the ones nothing had changed in. Those are left alone now, so saving a big project is quicker, and tools that go by a file's date (Git, backups, sync folders) see only what really changed.
- **npm packages stay installed when you switch bots**: all bots share one packages folder, and it held only the packages of the bot that ran last. Running another bot with other packages (a Fluxer bot after a Discord bot, one with a database, one with music) removed the first one's, and going back installed them again, every time. Packages now stay once installed; where two bots ask for different versions of one package, the bot that runs gets its own.

### Security
- **XC Circuit asks before it loads a project's own plugins**: a project can bring plugins in its `plugins/` folder, and they loaded as soon as it opened. With a project from someone else that was their code running on your computer, without a click: a plugin's editor script could start the bot, which runs the plugin's blocks with the same access as any program. Now **Load this project's plugins?** lists them first, once for each project folder and plugin; your answer is kept on your computer, not in the project. With Not now the project opens without them (their blocks show as not installed), and Plugins › Load This Project's Plugins… asks again. The example bot needs no asking. A project's plugin can no longer take the place of one that comes with XC Circuit (such as Discord Bot, which handles your token). After updating, your own projects with a `plugins/` folder ask once.
- **A plugin can't start the bot by itself**: the script of a plugin that doesn't come with XC Circuit can start the bot or act on its server (`xc.run("bot.run")`, `server.…`, `run.…`) only from one of its commands or from a click on its button, link or overlay, not from a timer, a check or while it draws.
- **The Console hides the bot's secrets**: a bot can be made to print its own token or a saved login (a Log block with `${process.env…}` is enough), and it then stood in the Console, where a screen share shows it, a plugin's script reads it and an AI agent gets it with `get_console`. Wherever one of the bot's secrets turns up in what the bot prints, in a block's values on a wire or in an error, `[hidden secret]` now stands instead, also when only its start shows. This is a safeguard, not a wall: code in your bot still has the token, as it must.
- **AI agents: clearer limits, said as they are**: the script an agent runs with `run_script` no longer gets the part of the plugin API that adds XC Circuit to AI clients' settings, nor the functions behind it (see below). The panel and the docs no longer say an agent "never sees" the token: no tool gives it out and the Console hides it, but an agent that builds and runs your bot works with a bot that has it.
- **Editor plugins can't read files outside their own folder** (Windows): a plugin's script loads its own files with `require("./file.js")`, and shows its own pictures and plays its own sounds. A name holding a drive, such as `./C:notes/keys.txt`, got past the check that keeps those inside the plugin's folder, so a plugin (or an AI agent, through `run_script`) could read text files elsewhere on the computer. Such names are refused now, and so is a link in the folder that points outside it. The functions behind `xc` are no longer there for a script to call directly.
- **Your server password, API key and bot token only go to the server you saved**: where a bot is uploaded to is kept in the project, so it comes along with a project from someone else, changes with a Git pull, and could be changed by a plugin's script or an AI agent; the password, key and token are kept on your computer. They went to whatever server the project named: another address got your saved password and, with Send the bot token on, the token. XC Circuit now remembers the server you saved in Bot › Server… for each project on this computer. When the project names another one, nothing is sent, the Console says so, and Bot › Server… shows a note: look it over and press Save to use it. Scripts and agents can no longer change the server, and `get_server` tells agents when it isn't the saved one (`savedHere`). After updating, a project with a saved password or key keeps working as it is; one that uploads with a key file alone asks you to press Save once.
- **HTTP Request's "public web only" holds for IPv6 addresses too**: an address from `${…}` or a wire may only go to the public web, not to the bot's own computer or network. Written as an IPv6 address with an IPv4 one inside it (`http://[::ffff:127.0.0.1]/`, or a cloud's `[::ffff:169.254.169.254]`), a private address counted as public. Addresses are now compared by their numbers, however they're written, including the older ways of putting an IPv4 address into an IPv6 one.
- **Links from plugins only open web pages**: a block's Docs button, and a build mode's invite page and developer portal, handed whatever text the plugin gave to the system to open, which opens a program or a file just as readily as a web page. They open `https://` and `http://` addresses only now; with anything else the button is greyed out and shows the text when you point at it.
- **The AI agents' connection is harder to get into**: the secret an AI client's connection needs is kept in a file only you can read (on Linux it could be read by other users of the computer, who could then steer the editor), and comes from the system's own random numbers. A program that connects and doesn't say the secret within 5 seconds is let go, a line without an end is cut off, and at most 32 connections are open at once.
- **An update is checked again just before it's installed**: a downloaded update waits on the disk until you close XC Circuit, and was then run as it was, from the temporary folder every program writes to. It's now kept in XC Circuit's data folder and checked against its checksum once more before it runs; a file that changed meanwhile is deleted instead. The app only updates itself with a file from its own releases on GitHub that has a checksum; otherwise it opens the release page, as before for other kinds of install.
- **A plugin's zip can't unpack to more than a plugin may be**: a small zip can claim to hold gigabytes, which would have filled the memory and the disk while it was installed. Installing stops at 64 MB for one file, 256 MB in all or 20,000 files (the bundled plugins are a few megabytes each), and leaves nothing behind.
- **A plugin only reads its own files**: the patterns in a plugin's `plugin.toml`, the files a block brings into the bot (`requires.assets`), a block's own icon and a build mode's runtime libraries could name files outside the plugin's folder, which then went into the bot and into exports. Those are left out now, with a line in Problems saying which.

### Fixed
- **The search (Ctrl+P) no longer stops the app on Turkish text**: looking for a word that stands near the end of a block's text with several İ in it closed XC Circuit. Such texts are searched like any other now, and typing `indirim` finds İndirim.
- **A panel too narrow for its controls no longer keeps the app busy**: a row with a dropdown, a slider or a number that didn't fit its panel (a plugin's panel in a narrow side bar) made XC Circuit draw itself again without end, which costs processor and battery for nothing. So did a settings page with two settings of the same name. Both come to rest now.
- **Release notes: square brackets before a link stay text**: in the update window, a line such as `An array [a, b] … see [the docs](…)` turned everything from the first bracket on into the link. Only the link is one now.
- **A Try Zone's border has the colour it was meant to have**: it was drawn about 40% darker than the theme's accent colour, because its see-through-ness was counted twice when colours were mixed. It's brighter now, and glows towards green as before when the zone runs.
- **A number field keeps to its `max` without a `min`**: a block's number field with only a `max` took any number. (None of the bundled blocks has one; blocks of other plugins may.)
- **A dropdown shows what it is really set to**: a dropdown in Bot › Settings or in a plugin's panel whose value was none of its choices (nothing chosen yet, or a choice the plugin no longer has) showed its first choice, as if that were set. It now says **Choose…** when nothing is chosen, and shows a value that isn't one of the choices as it is.
- **The bot's output no longer stops at an odd character**: a line that wasn't UTF-8 (a program the bot starts answering in Windows' own encoding, with an umlaut in it) ended the reading of everything the bot wrote, so the Console stayed empty from there on, blocks stopped lighting up, and the bot could end with a write error. Such characters now show as `�` and the rest is read as before. The same goes for npm's output while packages install, and for the database helper.
- **Installing Node.js can't hang on a talkative installer**: winget's or the installer's output and errors were read one after the other, so one that wrote much to its errors first waited forever. Both are read at the same time now.
- **An AI agent's request that took too long isn't done later after all**: while a dialog was open (a file picker, say), an agent's tool call waited, and after a minute the agent was told it failed. The call still ran once the dialog closed, so an agent that had tried again got its block twice. A call that was given up is now dropped, and the agent is told nothing was done.
- **Workspaces named Con, Nul, Aux, Prn, Com1 or Lpt1 can be saved**: Windows keeps those names for devices, so the workspace's file (`con.json`) couldn't be written and saving failed. Such a workspace now gets a file name of its own (`con-workspace.json`); its name in the side bar stays as typed.
- **Replacing a plugin can't lose it**: installing a plugin's zip over an installed version (or a block update) deleted the old version first; if the new one then couldn't be moved into place, the plugin was gone. The old version now steps aside and comes back when that happens. A folder an interrupted install left behind is no longer taken for a plugin.
- **Stop doesn't leave the bot's helpers running** (Windows): a bot that didn't stop within a moment was ended, but programs it had started (a player or converter for music, say) kept running. They're ended with it now.
- **A power cut right after saving can't leave an empty file**: a saved project file took the old one's place before its contents were surely on the disk, so a power cut at that moment could leave it empty. It's written to the disk first now. The same goes for backups and the crash recovery copy.
- **Block source shows the file instead of running it**: the Inspector's Block source opened a block's `.js` file with whatever Windows opens such files with, which is its script host unless you chose an editor: an error box at best. It now shows the file in its folder, to open with the editor you like.
- **Message previews: a link without a text is no link**: `[](http://…)` was shown as a link (an empty one) while `[](https://…)` stayed as typed. Both stay as typed now, as in Discord.
- **Slash Commands: a pasted command for administrators only stays one**: in Discord's JSON, `"default_member_permissions": "0"` means nobody but administrators can use the command. Pasted into the builder it became a command for everyone. It now comes in with Administrator ticked, which says the same.
- **Slash Commands: editing a command as JSON keeps its folder and its switch**: a command changed in the JSON view left its side bar folder, and one that was turned off was on again, with the first valid edit. Both stay as they were.
- **Slash Commands: Smallest and Largest take negative numbers and decimals**: the boxes wrote the number out again after every key, so a minus typed first vanished and so did the point: `-5.2` ended up as 52. They now keep what you type until the number is whole. A choice's number shows as typed, too (1, not 1.0).
- **Embed Builder: "When it's sent" is no longer lost after another embed had a set time**: once "A set time" was picked for one embed's timestamp, the next embed opened showed "A set time" too, with an empty box, and its own "When it's sent" was gone when you pressed Apply. Each embed keeps its own now.
- **Layout Builder and Modal Builder no longer stop the app on parts that aren't objects**: a message whose media gallery listed its pictures as plain addresses (`"items": ["https://…"]`), or a select menu whose options were plain texts, closed XC Circuit as soon as the builder showed it. Pasted JSON like that is now refused with what's wrong. A block that already holds such a layout (put there by hand or by an AI agent) opens in the JSON view with the same note, instead of an empty builder.
- **Slash Commands and the side bar no longer undo each other**: with Slash Commands open, a click on a command in the side bar read every command afresh and threw away changes you hadn't saved yet. And a command turned off, moved to a folder or dragged elsewhere in the side bar while the builder was open came back as it was with the builder's next Save. Now the click only shows that command, and the open builder takes over what the side bar changes.
- **Block picker: the keyboard works while the mouse rests on the list**: the row under the pointer took the highlight back in every frame. With the pointer over the list (where the picker often opens), the arrow keys did nothing, and Enter after typing a search placed the block under the pointer instead of the best match. A row is now highlighted when the pointer moves onto it.
- **Canvas Builder: a locked layer can't be resized by dragging**: its corner handle was hidden, but dragging where it would be still resized the layer.
- **A wire picked up and dropped on a pin it doesn't fit goes back**: it was deleted, as if dropped on empty canvas. Dropped on empty canvas it is still deleted.
- **New Project reads its starters once**: while the window was open, every starter's files were read from the disk again for each frame drawn (60 times a second), and the folder the project goes in was looked up as often, which could hold the window up with a location on a slow or unplugged network drive. The starters are read when the window opens, the folder when you change the name or location and about once a second.
- **A very long line in the Console no longer slows the app down**: a line of hundreds of thousands of characters (a big JSON printed by the bot, say) was measured again from its start for each row it wraps into, for every frame the Console was drawn. With a line of a million characters that took a tenth of a second per frame. It's done in one pass now.
- **Canvas Builder: the preview reads a colour of digits as the bot does**: a colour written as six digits without `#` (such as `112233`, from a preview value) was shown as another colour than the bot draws, because the preview took it for a plain number. It's read as hex, like the bot does.
- **Layout Builder and Modal Builder: a new part's custom id isn't one that's taken**: new buttons, menus and questions were numbered by how many parts there were, so after removing one the next got an id another already had, and "used twice" showed up right away. They're numbered one above the highest now.
- **Stored Data doesn't save over a store.json it couldn't read**: when the bot's `store.json` was there but couldn't be read (held by another program for a moment, or cut off), the panel showed "Nothing stored yet", and adding a value then saved a file with only that value over everything the bot had stored. The panel now says the file can't be read and changes nothing until it can.
- **The colour picker remembers a colour also when closed with its own button**: a colour showed up under Recent when the picker was closed with Escape or a click elsewhere, but not with a click on the swatch that opened it.
- **Builders' JSON view: Clear no longer leaves "Applied" standing**: after Clear the box was empty, but the line above it still said "Applied" and Apply put the earlier design on the block. An empty box now says "Paste JSON here", and Apply waits for valid JSON.
- **More of the interface is translated**: some texts stayed English in other languages: problems such as "needs a value" and "is not installed", the Backups page's first lines, the block picker's "by … · In: … · Out: …", the Slash Commands builder's notes on names and descriptions, "connect" and "Caught: …" on the canvas, and a few error lines in the Console. They're translated now (German comes with it).

## 0.2.9

### Added
- **Channel Info** (Discord and Fluxer): reads one thing about a channel, as Role Info and Server Info do for roles and servers: its name, id, mention, kind, topic, creation date, server, category, position, age restriction, slowmode, the channels in a category, its last message and pinned messages, and a voice channel's bitrate and user limit. On Discord also its link, members, active threads and forum tags, and for a thread the channel it's in, who started it, and whether it's archived or locked.
- **Collection to List**: turns a Collection (a Discord library's members, roles, channels or messages by id), a Map, a Set or an object into a list: its items, their keys (ids), or both. Blocks already give lists; this is for values from JavaScript, such as `${message.mentions.users}`.

### Changed
- **Blocks give lists, never Collections**: a Collection a block gives (Get Property reading `members.cache`, a block's own JavaScript, an event) is now a list of its items, and so is one that reaches an input for a list. List Tools, For Each and the rest used to get `[id, item]` pairs from a Collection; they get the items now. Inside `${…}` a Collection stays one, with `size`, `first()`, `get(id)` and the rest.
- **Layout and settings are saved a moment after a change**, not every 30 seconds: 5 seconds after the last change (a click, a key, the window moved or resized, a plugin storing something), and each change in the meantime starts the wait again. Nothing changed, nothing is saved. Settings › Editor › **Save layout and settings after** sets the wait (1 to 60 seconds); it replaces Save layout and settings every.

### Fixed
- **No more short freezes every 30 seconds**: when it saved the window's layout and settings (every 30 seconds, Settings › Editor), XC Circuit also saved everything egui, which draws the window, remembers, with the window locked meanwhile. That included the place of every popup, menu and window ever shown, start after start, so it grew into thousands and the window could stop answering for a few seconds each time ("XC Circuit didn't answer for 3.6 s … inside egui" in the Console). It now keeps only what's worth remembering (which sections are open, a text field's height …) and writes it with the window free.

## 0.2.8

### Added
- **More options on blocks**: options used less often (Delay, Get Rows' Sort by, Get Stored Value's If missing …) fold away below a **More options** row at the bottom of a block's fields; click it to open them, and each block remembers. Folded, an option that's in use (connected, a pin, or set to something) still shows, so nothing set is hidden. This replaces 0.2.5's Settings › Editor › Advanced options on blocks; the Inspector lists them under More options.
- **Settings › Window › Accessibility** (on to start with): screen readers and the touch keyboard can see what's in the window again. 0.2.6 left that out, as programs that read windows were suspected of making the app stop answering; if that happens, this turns it off while XC Circuit runs, and freezes.log says whether it was on.
- **AI agents can zoom**: the `screenshot` and `show_in_editor` tools take a `zoom` in percent (25 to 200), so an agent can look closer, or see the canvas at the zoom you mention. Plugins get it through `xc.reveal(workspace, block, zoom)`.

### Changed
- **Events give their objects first**: Select Menu Used, Button Pressed, Message Sent and the other events now list their Discord and Fluxer objects (Interaction, User, Member, Channel, Message, Server …) at the top of their outputs, and plain values (Value, Custom id, Rest of id, ids and texts) below. Wires already connected stay connected.
- **Send Message shows what it sends to first**: its Interaction, Channel, User or Message (and the layout, for Layout messages), which its settings add, came at the very bottom of the block, below every option, where it was easy to miss. They're now at the top. Blocks keep their own order otherwise; a plugin's `define()` can place a field it adds with `after` (a field's key, or `""` for the top).

### Fixed
- **The "connect" hint no longer vanishes at some zooms**: zoomed out to around 85%, the grey "connect" next to inputs such as Embeds and Components turned almost black, because making small grey text brighter counted its see-through-ness twice. It keeps its colour now, and gets brighter as meant when zoomed further out.
- **One slow moment doesn't switch off a kind of block**: since 0.2.7, a block whose `define()` took over 50 ms even once (a virus scan or a busy computer can cause that) was skipped for the rest of the session, so all blocks of that kind (it could be Send Message) lost the fields and pins they add. Now that only happens after 3 times in a row, a call in time starts the count again, and a block that was stopped once is worked out again instead of kept without its pins.
- **Save and close waits for the save**: since saving went into the background, a save that failed while closing went unnoticed and the changes were lost. XC Circuit now waits for the files, and if saving failed it stays open with your changes and says so in the Console.
- **No false "it was frozen" after closing**: waiting while closing (for the save to be written, or the bot to stop) could be counted as a freeze, and the next start then said the run before ended stuck.
- **Undo steps aren't lost while moving the slider**: dragging Settings › Editor › Undo steps down dropped the older undo steps at once, even when it went back up. They now go with the next change.
- **Settings › Bot starts Node.js less often**: it asked Node.js for its version every 3 seconds while the page redrew; now once a minute (Check again asks at once).

## 0.2.7

### Fixed
- **Saving no longer freezes the window**: saving, the backup made with each save and the crash recovery copy (every 30 seconds while there are unsaved changes) were written while the window waited. On a slow or busy disk, or with a virus scanner checking each new file, that could make Windows say "Not Responding". They're now written in the background: the status bar says "Saving…" until the files are written, and if saving fails, the project shows as unsaved again.
- **No more waiting on the credential store**: the bot token and other secrets were read from Windows Credential Manager (or the Linux keyring) while the window waited, when opening a project, Bot › Settings, Export, the Databases page, and on every Run. That's usually quick, but can take seconds. They're now read in the background when a project opens and kept in memory while it's open, so these open at once.
- **Blocks are worked out faster**: working out a block's pins (its `define()`) cleaned up the block sandbox's memory after every single block, which took longer than most blocks themselves. It now does so now and then, so opening a big workspace, or changing slash commands or Bot › Settings (which works out every block again), takes about a third less time, and up to three quarters less for smaller ones.
- **A slow block can't freeze the window any more**: a block from a plugin whose `define()` runs too long is stopped after 50 ms, but that happened again for every block of its kind, each time something changed, so a few dozen of them could freeze the window for seconds. Now it's stopped once: blocks of that kind show only their fixed fields and pins until the plugin is reloaded, and the Console says which block it was.

## 0.2.6

### Added
- **Settings for slower computers and big projects**: Settings › Editor › Save layout and settings every (30 seconds to start with, 10 seconds to 10 minutes) sets how often XC Circuit saves its panel layout and settings while it runs; they're still saved whenever it closes. Settings › Editor › Undo steps (200 to start with, 10 to 1000) sets how many changes each workspace can undo: every step keeps a copy of the workspace, so fewer steps use less memory. Settings › Backups › Crash recovery now sets how often unsaved changes are copied (every 30 seconds to start with).

### Changed
- **No screen reader support for now**: XC Circuit no longer offers its window to screen readers and other programs that read what's in windows (UI Automation on Windows, AT-SPI on Linux). Programs that do this (accessibility tools, some password managers and writing helpers) may be what makes it stop answering after a while, so it's off while that's looked into.

### Fixed
- **No more "Not Responding" during setup and the tour**: the first start page, the tour's Run step and Settings › Bot asked Node.js and winget for their version while drawing, and waited for the answer. Starting them can take a long time (the first time after installing, a virus scan, a slow virtual machine), and every 3 seconds in the tour, so the window froze for up to a minute at a time. They're now asked in the background, and the window keeps working.
- **freezes.log says more**: before the first freeze of a start, it says how the app draws (and whether in software), whether it's the x86_64 or ARM build and, on Windows, the processor. The first start page, the side bar, the tour's card and the block picker now count as places of their own, so a freeze there is no longer blamed on the menus or the Console.
- **Freezes the app never comes back from are no longer lost**: the Console only told of a freeze once XC Circuit answered again, so one it was closed in never showed. The next start now says it in the Console. After ten seconds stuck, freezes.log says so again and, on Windows, a dump of exactly where it hangs (every thread's stack, a few MB) is kept in the freezes folder next to it, the last three. Saving the window's layout every 30 seconds, and egui getting a frame ready, count as places of their own too.

## 0.2.5

### Added
- **Action Row**: a block for one row of a message's buttons (up to 5) or a select menu, so you choose which buttons share a row: connect them to it, and it to Send Message's Components. Buttons without one still fill rows of five in order.

### Changed
- **Modal Submitted knows the form**: each input of the form with its custom id (made in the Modal Builder) is an output by itself, of its kind: a box gives its text, a menu of options the values picked, a user, role or channel menu the members, roles or channels picked, an upload its files (as lists). No more typing the ids into Input ids, which is now only for forms made from blocks (ids there still give text, as before). The Modal Builder's Copy for Input ids is gone.
- **Every option on the blocks**: options that used to hide in the Inspector under Advanced (Get Stored Value's If missing, Get Rows' Sort by and At most, a block's Delay …) now show on the blocks too, and the Inspector lists them with the others. Blocks with such options get taller, so in older projects one may now reach over the block below it. Settings › Editor › Advanced options on blocks turns it off again.

## 0.2.4

### Added
- **For older computers**: Settings › Window › Lighter drawing is a list to tick, each part on its own or All at once: No shadows, No fading, Still glows (empty required fields and blocks that just ran glow without pulsing, so the app doesn't draw itself again 30 times a second) and Simpler edges (about 55–65% fewer triangles on the canvas). All of them are on by themselves when the app draws without a graphics card (and the Console says so), unless All without a graphics card is unticked. Settings › Window › Graphics picks Automatic, OpenGL or Older OpenGL from the next start, for graphics chips that can't do DirectX 12 or Vulkan.

### Changed
- **Notes readable from afar**: zoomed out, a note's title and description no longer shrink to nothing while its title band grew; they stay readable on screen, like a map's labels, so notes name the parts of a big workspace. A long description keeps its own size when a bigger one would cover the blocks, and the title band no longer gains empty room above the title.
- **One Update button for everything**: in the update window (at start, or from Help › Check for Updates), the button at the bottom right now updates the blocks and the app together. XC Circuit restarts only when the app itself updates, after the blocks are in; blocks alone still update without a restart.
- **Long wires**: holding a wire you're dragging at an edge of the canvas (or just past it) moves the canvas that way, faster the closer you get, so a wire can reach a block that's out of view.
- **A lighter canvas background**: the dots are one repeated picture instead of a small circle each, which took up to a third of a canvas frame and 20,000 triangles. They look the same.
- **A freeze detector**: when XC Circuit stops answering (Windows says "Not Responding": it can't reach the window; one that only isn't drawing while it's moved or minimized doesn't count) for three seconds or more, the Console says for how long and where it was stuck (a tab, saving, the Problems list, plugins, or after its frame: the graphics card, Windows or a tool reading the window), and freezes.log in the app's data folder keeps it, also while it's still stuck. It also says when the app was stuck inside egui (laying out a long text, say, or the window's accessibility tree). It helps find what makes slower computers freeze.

### Fixed
- **Pasted blocks point at each other**: a text such as `${#7.user.id}` kept pointing at block #7 after copying and pasting, though the pasted blocks get new numbers, so in another workspace it read a block that isn't there ("Cannot read properties of undefined"). Pasting (and Duplicate) now points it at the pasted copy of #7 when that was copied along, in texts, embeds and layouts alike. Problems now also lists a text that uses a block number the workspace doesn't have, for blocks pasted before this fix.

## 0.2.3

### Added
- **Collections and Maps in ${…}**: after a dot, a Collection (a server's members.cache, a member's roles.cache, a message's attachments …) lists size, first(), last(), get(id), find(…), filter(…), random() and more, and after one that gives back an item, that item's members: ${server.members.cache.get(user.id).displayName}. Managers' caches now know what they hold. A Map lists size, get and has.
- **Builder windows keep their place**: the Embed, Layout, Modal and Canvas Builders, Slash Commands, Bot settings and the other editors open where they were the last time and as big (moved inside the window if it got smaller). A new window no longer turns up where an old one was in an earlier run.
- **AI agents see much more**: new MCP tools list every block type (`list_block_types`), show a block's source code (`get_block_source`) and the plugins (`list_plugins`), read the app's settings and read or change Bot › Settings (`get_app_settings`, `get_bot_settings`, `set_bot_settings`; never the token), and see and work the bot's server (`get_server`, `server_action`: upload, start, restart, stop, kill; `set_run_target`). `run_bot` and `stop_bot` act on the server when Run does, and `get_project` tells about the server. Plugins get the same through `xc.server()`, `xc.botSettings()`, `xc.blockSource()` and more.
- **Defer, then delete**: a Send Message mode that answers a command, button or form with *thinking…* and takes it away again (after Delete after seconds, or as soon as it shows), so it counts as answered and nothing stays. A reply after it comes as a new message.

### Fixed
- **A form with a too short text in a box**: Discord refused the whole form ("Must be 7 or more in length") when a box's filled-in text was empty or shorter than the box's least characters, such as a ${…} that came out short. Show Modal now leaves that text out (the box starts empty, and the Console says so) and cuts one that is too long, and the Modal Builder warns about a typed text that doesn't fit.
- **The note editor**: the title and description boxes have room around their text and grow by the grip in their corner, Delete note has room around its text, and Other color picks any colour with the color picker (the presets stay).
- **Delete after works for replies**: Send Message deleted its message as a channel message, which Discord refuses for private replies and where the bot can't see the channel, and said nothing. Replies and follow-ups are now deleted through the interaction (for 15 minutes; later as messages), and a delete that fails says why in the Console.

## 0.2.2

### Added
- **More shortcuts**: Settings › Shortcuts now also takes keys for Start and Restart on their own, Bot settings, Slash Commands, Export, Upload to the server, Server console, Server settings and switching Run between this computer and the server (none set to start with). With Server picked, Start, Restart and Stop act on the server. Shortcuts with Ctrl, Alt or a function key also work in a tab's window of its own.
- **Tabs in windows of their own**: drag a tab outside XC Circuit's window (or right-click it › Open in its own window) and it opens in a window of its own, for a second monitor. It has no system frame (unless Use system title bar is on): its tab bar and tabs move it, a double-click maximizes it, and **Back into the app** puts the tab back.
- **Run the bot on its server**: with a Calagopus server set up, a Local | Server switch shows beside Run. On Server, Run uploads what changed and starts the server (Restart when it runs), Stop stops it, and the status bar shows the server's state (running, starting, stopping, stopped) with its CPU and memory. The choice is kept per project.
- **The builders check Discord's limits**: the Embed Builder now says what Discord would refuse (a title over 256 characters, a field value over 1024, more than 6000 characters in all, an address without https://, an empty embed), and the Layout, Modal and Slash Command Builders check more of Discord's limits: button labels and links, select menu placeholders, options and how many can be picked, picture descriptions, form boxes and file uploads, required options before optional ones, 25 subcommands and 8000 characters per command. Text in `${…}` isn't counted. Blocks with a design Discord would refuse show it in Problems, without opening the builder.
- **Colors from an input or a variable**: every color picker on blocks and in the Embed and Layout Builders (as already in the Canvas Builder) has a `${}` button beside it, for a color the bot works out, such as `${role.hexColor}` or `${color}`; each name gets an input on the block. Create Role's color can also be a pin to connect. Hex colors work with or without the `#` (#FF5CA8, FF5CA8, and short ones such as #F5A), and anything else stops the block with a clear message. Edit Role and Fluxer's Manage Role take colors without `#` too.

### Fixed
- **Send Message to a channel works after a form**: "target.send is not a function" came when an interaction was still wired from when the block replied (Send to channel used it instead of the channel), or the channel was kept as data (in a variable or Stored Data). Send Message now uses what its mode needs, a channel kept as data is found again by its id, and a channel that takes no messages (a category, a forum) says so.
- **No false "nothing handles the form my-form"**: Problems counted Show Modal's older Custom id, hidden while the form comes from the Modal Builder. Hidden fields no longer count.
- **Importing workspaces brings whole commands**: a command with subcommands (such as /ticket open and /ticket close) came in as /ticket alone, so its subcommands were missing. Now the command and all its subcommands come along, and the export and import lists show them (/ticket: open, close).
- **Signing in to a Calagopus panel again works**: the panel takes each API key name once, so a second sign-in was refused as "name already taken". The key is now named after the bot, with a short tag of its own (such as XC Circuit · Test Bot #a1b2).
- **A pasted Calagopus API key lists the servers**: they're read as soon as you stop typing, without Read again, and the server list says what's missing (the address, a key) or what went wrong instead of always "Sign in".

## 0.2.1

### Changed
- **The server console can go in the side bar**: drag its tab onto the activity bar or the side bar (either half), like the Console. In a narrow side bar its buttons go under the server's name.
- **The server console wraps long lines** at its width, so nothing is cut off at the side and there's no sideways scrolling.
- **No command line in the server console**: bots don't read typed commands, so the box under it is gone and the console gets the room.

### Fixed
- **The server console fits a small area**: made smaller, it shows fewer lines instead of keeping a least height and running past the bottom.

## 0.2.0

### Added
- **Two views in the side bar**: drag an activity bar icon (or a Workspaces, Stored Data, Plugins or panel tab) onto the lower half of the side bar to show it under the first, such as Workspaces above and Stored Data below; while dragging, the half it goes in lights up. Drag the line between them to share the height; right-click a half's name to swap, close it, or move it to the main area. Both are remembered.
- **Put the bot on its server**: Bot › Server… uploads the bot to the server it runs on, over SFTP, FTP (encrypted, FTPS) or a Calagopus panel, by hand or after every save (Upload after every save). Only changed files are sent, and the server's data/ folder is never touched. Passwords and keys stay in your system's credential store; the bot token goes along only while Send the bot token is on. The bottom bar shows how the last upload went.
- **Calagopus panels**: sign in through the browser (the panel makes an API key for XC Circuit and hands it back), choose the server, and uploads restart it so the new version runs. Bot › Server console shows its state, CPU and memory, Start, Restart, Stop and Kill, and its live console in its own colours (as in the panel), where you can type commands. Left open, it is there again (connected) the next time XC Circuit starts.
- **A full ticket system to start from**: the Tickets starter (File › New Project) is now a ticket system with Components V2: /ticket-panel posts a panel with a menu, picking a kind opens a private ticket channel with Close ticket and Transcript buttons, closing asks first, keeps a transcript in #ticket-logs and deletes the channel. It needs no ids: make a ticket-logs channel and a Support role (or pick yours in Find Role).
- **Pick channels and roles from a list**: with the bot token saved, id fields for channels, categories and roles (Send Message's channel, Create Channel's category, Find Role, Add Role …) get a button that lists your bot's servers with their channels or roles. Pick one and its id goes in: no Developer Mode and copying ids.
- **Custom ids are picked, not typed twice**: Button Pressed, Select Menu Used and Modal Submitted list the custom ids your layouts, forms and Button and Select Menu blocks use (Custom… for any other). Problems now says when an event waits for an id nothing has (a typo such as tiket-close) and when a button, menu or form has nothing to handle it.
- **No more "This interaction failed" on slow flows**: when the blocks after a slash command, button, menu or form take longer than Discord's 3 seconds, the bot answers for them in time (quietly for buttons and menus, with *thinking…* for commands and forms), and Send Message's reply then arrives the right way. No Defer block needed.
- **Fewer wires: the event's interaction and server**: an Interaction or Server input left unconnected takes the event's own, such as the slash command's interaction for Send Message's reply. The block shows **from the event** instead of *connect*. Users, members, channels and messages still need a wire, so an unconnected Ban can't hit the wrong person.
- **The invite link asks for what your blocks need**: Invite your bot now offers, first, the permissions the project's blocks need (Create Channel needs Manage Channels, Moderate's ban needs Ban Members …), and lists them, so the bot doesn't fail later for a missing permission.
- **Find Role, Find Channel and Find Member have a Missing path**: when what you look for isn't there, connect Missing to handle it (tell the user, use a default) instead of the whole flow stopping with an error. With nothing on Missing they fail as before. They also take mentions such as <@&id>, <#id> and <@id>.
- **Mentions made easy**: the values listed after `${` say which ones ping (users, members, roles) or link (channels), and hovering shows how: `${user}` in a text pings them, `${user.username}` is just the name. The Mention block takes typed ids and lists of people, and does @everyone and @here.
- **Split Text**: takes a text apart at spaces, commas or any separator and gives the first part, the last, one by number (-1 is the last), everything after the first, and how many there are: no more list blocks to get one argument of a command.
- **Dates made easy**: Date & Time adds any amount (`1d 2h`, `-30m`), writes your own pattern (`DD.MM.YYYY HH:mm`) in any time zone and gives every Discord timestamp style. **Read Date** turns typed text into a date (`24.12.2026 18:00`, `tomorrow 9:00`, `in 2h` …), and **Time Between** says how long lies between two dates, in days, hours and minutes or as text.
- **AI agents get XC Circuit's tools as soon as it opens**: an AI client that started while XC Circuit was closed only saw `open_xc_circuit` until it reconnected. The MCP server now tells the client when XC Circuit opens or closes, so its tools appear (or go) by themselves within a few seconds.
- **Folders in folders**: workspaces and slash commands can be sorted into folders inside folders. Drag a folder onto the middle of another to put it in there, onto its top or bottom edge to put it above or below, or onto the project (or Slash commands) row to bring it back to the top; a folder's menu has **New folder here**. Deleting a folder moves what's in it up a level. Projects keep their folders as they were.
- **Clearer dragging in the side bar**: moving workspaces, slash commands and folders shows what's dragged beside the pointer and what letting go does ("Into Events", "Below Startup, in Commands", in red where it can't go, such as a folder into its own subfolder). The dragged row stays faded in its place, folders being dropped into are outlined, the line between rows starts at the depth it lands at, a closed folder opens when you hold something over it, and the list scrolls near its edges.
- **More room for the builders' preview**: in the Embed, Layout and Modal Builders, **Hide** puts the values from earlier blocks away so the preview gets the whole column, and the line between the preview and the values can be dragged up or down. Both are remembered.
- **Windows resize at every edge**: the floating windows of the builders, Slash Commands, Bot settings and Export now resize at their top and left edges too, not only at the right and bottom.
- **Blocks fit their message**: blocks that show a message, embed or form (Build Layout, Build Embed, Show Modal …) get as wide as it is in Discord by themselves, up to Discord's widest (about 520 px for messages, 440 for forms): a row of buttons fits on one line, long text wraps where Discord wraps it. In your own projects, such a block can now reach into the one next to it: move that one a little.
- **Update the app from the Updates window**: when a new XC Circuit is out, Help › Check for Updates now has **Update and restart** right there: it downloads the update, installs it as XC Circuit closes (asking about unsaved changes first) and opens it again. **What's new…** shows the release notes.

### Changed
- **Emoji in colour**: emoji in the editor (previews, block texts, the Console) are drawn with the computer's own colour emoji, as in Windows and Discord, instead of black-and-white outlines. Symbols the editor uses as marks, such as ▶ and ⚠, stay plain.

### Fixed
- **Buttons show their Emoji field**: the Button block's Emoji was tucked away with the advanced settings, so it looked like buttons couldn't have one. It's a normal field now, and a button with an emoji can leave its text empty.
- **Better block search**: searching for *date* listed Member Updated and other events first, because *date* is inside *Updated*. Blocks whose name starts with what you typed come first now, then whole words, then the start of a word, then the description; a match inside another word comes last.
- **Buttons in a layout preview no longer squash**: in a row of buttons too wide for the block, the last button's text broke over several lines. Buttons keep their text on one line, as in Discord, and move to the next line when there's no room.
- **Plugin updates show their versions properly**: the list of plugin updates showed a box instead of the arrow between the old and new version. It now says "1.0.3 to 1.0.4".
- **Problems lists empty inputs a block adds itself**: inputs that appear from a block's settings, such as Send Message's Channel when sending to a channel or its Interaction when replying, weren't checked, so an empty one only glowed on the block. They're listed in Problems now too.
- **Blocks see slash commands an AI agent adds**: after an agent saved slash commands, Slash Command blocks still showed them as "not in the builder" until the project was opened again.

## 0.1.9

### Added
- **Voice Changed: tick several changes**: Change is now a list with checkboxes, so one event can run for joins and leaves (or any other mix). Projects that picked one change keep it.
- **Select several workspaces and delete them together**: Ctrl-click workspaces in the Workspaces list to select several, or Shift-click to select all from the last one clicked, then press Delete. It asks first, listing them. Clicking elsewhere (or Escape) lets go of the selection, so Delete on the canvas still deletes blocks.

### Fixed
- **${…} works in Check, Set Variable and other fields that take any value**: those fields kept ${…} as plain text, so Check compared the words `${#1.after.id}` instead of the id. They fill it in now, and a field that is only one ${…}, such as `${#1.after}`, gets the value itself (the channel), as a wire would give it.
- **Comparing Discord ids is exact**: Check, If and the other comparisons compared long ids (18-19 digits) as numbers, which can't hold that many digits exactly, so two different ids that differ only in their last digits could count as equal. Whole numbers that long are now compared exactly.
- **Send Message shows Components again**: since advanced pins start hidden, new Send Message blocks had no Components input for buttons and select menus. It's a normal input again. On a block made in between, click the small dot next to Components to make it a pin.
- **Multi-selects fit on the block**: with several things ticked (Voice Changed's changes, permissions), the box grew past the block's edge. Long picks now end in … and hovering shows them all.
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
