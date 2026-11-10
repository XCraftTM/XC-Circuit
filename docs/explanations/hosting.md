# Putting your bot on a server

A bot only answers while it runs. Running it from the editor is fine while you build it; to keep it online, it runs on a server: a VPS, a computer at home, or a hosting service with a game panel. XC Circuit calls that place the bot's **remote**. **Bot › Remote…** puts it there for you, and can do that again after every save.

## How it gets there

Pick one in **Bot › Remote…** (**Your host** is the [XC Circuit Host](host.md), which runs the bot itself; the others are servers and panels of other makers):

| | What you fill in | Good for |
| --- | --- | --- |
| **SFTP** | the server's address, port (usually 22; game panels often use 2022), user name, and a password or a key file | a VPS or any server you can log in to with SSH, and most game panels |
| **Calagopus panel** | the panel's address; then **Sign in with the panel** and choose the server | servers on a [Calagopus](https://calagopus.com) panel: uploading, Start, Restart, Stop and the live console |

**Folder on the server** says where the bot's files go (it's made when missing). Left empty, they go where you start after logging in, or, on a Calagopus server, into the server's own folder.

Passwords and API keys are kept in your system's credential store, like the bot token (see [Tokens and safety](./safety)). The other settings are kept on your computer, in the project's `.circuit/local.json`: they are yours, not the bot's, so they aren't shared when you share the project or put it into Git. On another computer you fill them in once.

Your password, API key and the bot token only go to the server you last **saved** in Bot › Remote… on this computer: when that file names another one (another address, user, folder, the token turned on), because something else changed it or the project came with it, nothing is sent, the Console says so, and Bot › Remote… shows a note. Look the settings over and press **Save** to use them. Plugins' scripts and AI agents can't change the server.

## What is uploaded

The same files as [Export](./running#what-export-does): `index.js`, `package.json`, `.gitignore`, `README.txt`, and `.env` with the bot token and secrets while **Send the bot token** is on. Turn that off to fill in `.env` on the server yourself.

Only files that changed since the last upload to the same place are sent; uploading an unchanged bot sends nothing. Nothing else on the server is touched: its `data/` folder (what the bot saved) and `node_modules/` stay as they are.

The server needs Node.js 22 or newer. It runs `npm install` itself: the Node.js servers of game panels do that when they start; on your own server, run `npm install` once, then start it with `node index.js` (or a service manager such as pm2 or systemd).

## Uploading

- **Save and upload** in Bot › Remote…, **Bot › Upload to the remote**, or the item at the bottom right of the window (it also shows how the last upload went: click it for the details).
- With **Upload after every save** on, Ctrl+S also puts the new version on the server.
- On a Calagopus server, **Restart the server after uploading** (on at first) restarts it, so the new version runs right away. On other servers, restart the bot yourself.

**Test connection** logs in and out again, to check the settings without uploading.

## SFTP: the server's key

The first time XC Circuit connects to an SFTP server, it keeps the server's key (its fingerprint shows in the upload's messages). Later connections check that the server still shows the same key. If it shows another one, the upload stops: that can mean the address now leads to another computer. If your server really got a new key (it was reinstalled), remove its line from `known_hosts.json` in XC Circuit's data folder and upload again.

## Calagopus

**Sign in with the panel** opens the panel's page for making an API key in your browser, with the permissions XC Circuit needs: reading your servers, their console, Start, Stop and Restart, and writing files. Approve it there, and the panel hands the key back to XC Circuit (on a one-time address on your computer). The key is named after the bot, such as *XC Circuit · Test Bot #a1b2* (the tag keeps it apart from earlier keys, as a panel takes each name once). You can also make a key yourself in the panel (Account › API Keys) and paste it: it needs the servers.read permission, and the servers are listed as soon as it's in. Then choose the server in the list.

**Bot › Remote console** shows the chosen server: its state, CPU and memory, **Start**, **Restart**, **Stop** and **Kill** (ends it at once, without letting it save), and its live console, in the colours the server prints (as in the panel). Like the Console, it can be a tab anywhere or an icon in the activity bar, shown in the side bar (drag its tab there). Left open when you close XC Circuit, the console tab comes back, connected, the next time it starts with that project.

### Running the bot on the server

With a Calagopus server chosen and signed in, a **Local | Remote** switch shows beside **Run** at the top right. Pick **Remote**, and Run, Restart and Stop (the buttons, F5 and Shift+F5, the Bot menu and the status bar) act on the server instead of this computer:

- **Run** uploads what changed, then starts the server, so it runs the version you see. When it runs already, the button says **Restart**: it uploads, then restarts it.
- **Stop** stops the server.
- The canvas shows which blocks run there, how long they take and where one fails, as for a bot run from the editor. The bot is asked to say so only while the editor watches (through the panel's console, a minute at a time), hides its secrets itself, and leaves out what goes through the wires unless **See the values on the wires too** is on. It needs the bot uploaded by this version or a newer one.
- The status bar shows the server's state (*Remote running*, *starting*, *stopping*, *stopped*), read every few seconds; hover it for the server's name, CPU and memory.

Picking Remote stops the bot running here, so the two don't both answer. The switch is kept per project; **Local** puts Run back on this computer (the server keeps running).
