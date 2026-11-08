# Putting your bot on a server

A bot only answers while it runs. Running it from the editor is fine while you build it; to keep it online, it runs on a server: a VPS, a computer at home, or a hosting service with a game panel. **Bot › Server…** puts it there for you, and can do that again after every save.

## How it gets there

Pick one in **Bot › Server…**:

| | What you fill in | Good for |
| --- | --- | --- |
| **SFTP** | the server's address, port (usually 22; game panels often use 2022), user name, and a password or a key file | a VPS or any server you can log in to with SSH, and most game panels |
| **FTP** | the address, port (usually 21), user name and password; **Encrypted (FTPS)** is on | web hosts and panels that only offer FTP |
| **Calagopus panel** | the panel's address; then **Sign in with the panel** and choose the server | servers on a [Calagopus](https://calagopus.com) panel: uploading, Start, Restart, Stop and the live console |

**Folder on the server** says where the bot's files go (it's made when missing). Left empty, they go where you start after logging in, or, on a Calagopus server, into the server's own folder.

Passwords and API keys are kept in your system's credential store, like the bot token (see [Tokens and safety](./safety)). The other settings are kept in the project, so you can upload again after opening it on another day.

## What is uploaded

The same files as [Export](./running#what-export-does): `index.js`, `package.json`, `.gitignore`, `README.txt`, and `.env` with the bot token and secrets while **Send the bot token** is on. Turn that off to fill in `.env` on the server yourself.

Only files that changed since the last upload to the same place are sent; uploading an unchanged bot sends nothing. Nothing else on the server is touched: its `data/` folder (what the bot saved) and `node_modules/` stay as they are.

The server needs Node.js 22 or newer. It runs `npm install` itself: the Node.js servers of game panels do that when they start; on your own server, run `npm install` once, then start it with `node index.js` (or a service manager such as pm2 or systemd).

## Uploading

- **Save and upload** in Bot › Server…, **Bot › Upload to the server**, or the item at the bottom right of the window (it also shows how the last upload went: click it for the details).
- With **Upload after every save** on, Ctrl+S also puts the new version on the server.
- On a Calagopus server, **Restart the server after uploading** (on at first) restarts it, so the new version runs right away. On other servers, restart the bot yourself.

**Test connection** logs in and out again, to check the settings without uploading.

## SFTP: the server's key

The first time XC Circuit connects to an SFTP server, it keeps the server's key (its fingerprint shows in the upload's messages). Later connections check that the server still shows the same key. If it shows another one, the upload stops: that can mean the address now leads to another computer. If your server really got a new key (it was reinstalled), remove its line from `known_hosts.json` in XC Circuit's data folder and upload again.

## Calagopus

**Sign in with the panel** opens the panel's page for making an API key in your browser, with the permissions XC Circuit needs: reading your servers, their console, Start, Stop and Restart, and writing files. Approve it there, and the panel hands the key back to XC Circuit (on a one-time address on your computer). You can also make a key yourself in the panel (Account › API Keys) and paste it. Then choose the server in the list.

**Bot › Server console** shows the chosen server: its state, CPU and memory, **Start**, **Restart**, **Stop** and **Kill** (ends it at once, without letting it save), and its live console, in the colours the server prints (as in the panel). Type a command and press Enter to send it, as in the panel.
