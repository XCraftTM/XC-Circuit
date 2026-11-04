# Tokens and safety

## The bot token never goes into the project

A bot token is a password: whoever has it controls the bot. XC™ Circuit keeps tokens and other secrets (settings marked `secret` by a build mode, such as music service logins) in your operating system's credential store:

- **Windows**: Credential Manager
- **Linux**: the Secret Service (GNOME Keyring, KWallet)

The project folder never contains them, so you can put a project in Git, share it or send it to someone for help without leaking the token. In Bot › Settings the eye button shows what you're typing, and Copy copies the saved token without showing it.

An exported bot reads its token from `.env`. Export only includes the token there when you ask it to, and its `.gitignore` keeps `.env` out of Git.

## What editor plugins can do

[Editor scripts](/api/editor) run in a sandbox (QuickJS) inside the editor:

- They reach the app **only** through the `xc` object: the open project, panels, commands, the Console, their own small storage.
- They have **no** access to files, the network, other programs or your secrets.
- Each call has a **time limit** (an endless loop is stopped) and each plugin a **memory limit**.

That's why they start without asking. [Native plugins](/api/native) have none of these limits, so they need your explicit approval.

## What bot code can do

Blocks run in your bot, which is a normal Node.js program. A block can read files, use the network and start programs, because bots need to. Treat block plugins like npm packages: use ones from people you trust, and read the code of ones you don't know. Blocks are plain JavaScript files, so you can.
