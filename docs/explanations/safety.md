# Tokens and safety

## The bot token never goes into the project

A bot token is a password: whoever has it controls the bot. XC™ Circuit keeps tokens and other secrets (settings marked `secret` by a build mode, such as music service logins) in your operating system's credential store:

- **Windows**: Credential Manager
- **Linux**: the Secret Service (GNOME Keyring, KWallet)

The project folder never contains them, so you can put a project in Git, share it or send it to someone for help without leaking the token. In Bot › Settings the eye button shows what you're typing, and Copy copies the saved token without showing it.

An exported bot reads its token from `.env`. Export only includes the token there when you ask it to, and its `.gitignore` keeps `.env` out of Git.

The passwords and API keys for uploading the bot to its server (Bot › Server…) are kept in the credential store too. An upload sends the token in `.env` only while **Send the bot token** is on; over FTP, keep **Encrypted (FTPS)** on, as plain FTP sends it unencrypted. They, and the token, only go to the server you saved in Bot › Server… on this computer, not to one that came with a project's files. See [Putting your bot on a server](./hosting).

## What editor plugins can do

[Editor scripts](/api/editor) run in a sandbox (QuickJS) inside the editor:

- They reach the app **only** through the `xc` object: the open project, panels, commands, the Console, their own small storage.
- They have **no** access to files, the network, other programs or your secrets.
- Each call has a **time limit** (an endless loop is stopped) and each plugin a **memory limit**.

That's why installed plugins' scripts start without asking. A script of a plugin that doesn't come with XC Circuit can start the bot, or upload it, only when you click one of its buttons or pick one of its commands, not by itself.

## Plugins that come with a project

A project can bring plugins of its own, in its `plugins/` folder: handy for blocks made for one bot, but with a project from someone else that is their code. XC Circuit asks before it loads them (**Load this project's plugins?**), once for each project folder and plugin, and remembers the answer on your computer, not in the project. Until you say yes, nothing of those plugins is read or run. A project's plugin never replaces one that comes with XC Circuit, such as Discord Bot, which handles your token.

## What bot code can do

Blocks run in your bot, which is a normal Node.js program. A block can read files, use the network and start programs, because bots need to. Treat block plugins like npm packages: use ones from people you trust, and read the code of ones you don't know. Blocks are plain JavaScript files, so you can.
