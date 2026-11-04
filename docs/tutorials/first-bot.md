# Your first bot

A Discord bot with a `/ping` command that answers **"Pong! 42 ms"**. About 15 minutes.

You need XC™ Circuit ([download](/download)) and a Discord server where you may add bots.

## 1. Make the bot in Discord

1. Open the [Discord Developer Portal](https://discord.com/developers/applications) › **New Application**, name it *Ping Bot*.
2. **Bot** › **Reset Token** › copy the token. It's a password: don't share it.
3. **OAuth2 › URL Generator**: tick **bot** and **applications.commands**, then **Send Messages**. Open the link at the bottom and add the bot to your server.

## 2. Create the project

**File › New Project…**

![The New Project window](/screens/new-project.png)

1. Pick **Discord Bot**.
2. Type a name.
3. Keep **Empty project** (the others are finished bots to learn from).
4. **Create**.

## 3. Paste the token

**Bot › Settings…**

![Bot settings](/screens/bot-settings.png)

1. Paste the token.
2. **Save and close**.

::: tip Safe
The token is kept in your computer's password store, never in the project folder.
:::

## 4. Make the `/ping` command

**Bot › Slash Commands…**

![The Slash Commands window](/screens/slash-commands.png)

1. **+ New command**.
2. Name `ping`, description *Checks that the bot answers*.
3. **Save and close**.

## 5. Add the blocks

Right-click the empty canvas (or press **Space**).

![The block picker](/screens/block-picker.png)

1. Type `slash`.
2. Click **Slash Command**.

Then add **Send Message** the same way, and connect them:

![The finished blocks](/screens/ping-blocks.png)

1. Set **Command** to `/ping`.
2. Drag from the green arrow on Slash Command to Send Message.
3. Drag from **Interaction** to **Interaction**: this tells Send Message what to answer.
4. Type the text:

```
Pong! ${Date.now() - interaction.createdTimestamp} ms
```

`${…}` is JavaScript: here, how long ago the command was used.

## 6. Run it

![The Run button](/screens/run.png)

1. **Run** (or **F5**). The first run installs discord.js, which takes a moment.

Type `/ping` in your server. The bot answers, and the blocks light up as they run.

::: details It doesn't answer?
- The Console says why, and the failing block turns red.
- No token: step 3. Bot not in the server: step 1.3.
- New command not showing yet: save with **Ctrl+S**, which restarts the bot.
:::

**Ctrl+S** saves. **Shift+F5** stops the bot.

Next: [Values and texts](./values-and-texts).
