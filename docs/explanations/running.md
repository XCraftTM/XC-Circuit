# Running and exporting

## What Run does

Your bot is a Node.js program. When you press **Run** (F5), XC Circuit:

1. Makes sure **Node.js 22 or newer** is there (the first start page sets it up).
2. Installs the npm packages the build mode and its blocks need (discord.js, @fluxerjs/core, and any a block lists in `requires.dependencies`), once, into a shared packages folder.
3. Bundles the runtime, your project and the blocks it uses into a run folder in the project's `.circuit/` folder.
4. Starts Node.js with the bot token from the credential store, and stays connected to it.

While the bot runs, the editor shows what it does: blocks glow when they run, failures turn them red, logs go to the Console, and the Stored Data view shows the bot's data. **Save** (Ctrl+S) restarts a running bot with your changes. **Stop** (Shift+F5) ends it.

The bot runs as its own process: if a block crashes Node.js, the editor is unaffected, and the editor can't freeze the bot.

## Answering in time

Discord waits 3 seconds for an answer to a slash command, button, select menu or form, and then shows *This interaction failed*. When the blocks after one take longer (creating a channel, a transcript, a web request), the bot answers for them after 2.5 seconds: a button or menu quietly (its message stays as it is, and Send Message's reply comes as a new message), a command or form with *thinking…*, which Send Message's reply then fills in (a private reply comes privately, and *thinking…* goes away). So **Defer** is only needed to show *thinking…* at once. A form (Show Modal) can't come after that: put it before slow blocks.

## What Export does

**Bot › Export…** writes a folder that runs on its own, without XC Circuit:

| File | What it is |
| --- | --- |
| `index.js` | the runtime, your project and its blocks in one file |
| `package.json` | the npm packages the bot needs |
| `.env` | the bot token and other secrets, if you chose to include them; otherwise empty lines to fill in |
| `.gitignore` | keeps `.env`, `node_modules/` and `data/` out of Git |
| `README.txt` | how to start it |

To start an exported bot anywhere Node.js runs (your PC, a server, a hosting service):

```bash
npm install
node index.js
```

The token is read from `.env` (`DISCORD_TOKEN=…` for Discord bots, `FLUXER_BOT_TOKEN=…` for Fluxer bots). Stored data goes into `data/`. Exporting again into the same folder keeps what you filled into `.env`.

To put it on a server and keep it up to date there without copying files by hand, see [Putting your bot on a server](./hosting).

Exported bots are yours: you may run, share and sell them. See the [licence](https://github.com/XCraftTM/XC-Circuit/blob/main/LICENSE).
