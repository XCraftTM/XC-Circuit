# Your first bot

In this tutorial you build a Discord bot with a `/ping` command that answers "Pong!" and says how long the answer took. You'll create a project, register a slash command, connect two blocks and run the bot.

You need XC™ Circuit ([download](/download)) and a Discord server where you may add bots (your own test server is best).

## 1. Create the bot in Discord

1. Open the [Discord Developer Portal](https://discord.com/developers/applications) and choose **New Application**. Give it a name, for example *Ping Bot*.
2. Open **Bot** on the left and choose **Reset Token**. Copy the token: you'll paste it into XC Circuit in a moment. Treat it like a password.
3. Open **OAuth2 › URL Generator**. Tick the scopes **bot** and **applications.commands**, and under Bot Permissions tick **Send Messages**.
4. Open the generated link at the bottom and add the bot to your test server.

## 2. Create the project

1. In XC Circuit choose **File › New Project…**.
2. On the left, pick **Discord Bot**. Type a name (*Ping Bot*), keep the location, and keep the starter on **Empty**.
3. Choose **Create**. The project opens with one empty workspace.

::: info Projects are folders
A project is a folder with a `circuit.json` and a `workspaces/` folder holding one JSON file per workspace. You can keep it in Git like code. See [Projects and workspaces](/explanations/projects).
:::

## 3. Add the token

Choose **Bot › Settings…**, paste the token into **Bot token** and save. The token goes into your computer's credential store (Windows Credential Manager or the Linux Secret Service), never into the project folder, so you can share the project without sharing the token.

## 4. Register the command

Slash commands are made in **Bot › Slash Commands…**:

1. Add a command named `ping` with the description *Checks that the bot answers*.
2. Save. Commands are registered with Discord each time the bot starts.

## 5. Connect the blocks

1. Press **Space** on the canvas (or right-click empty canvas) to open the block picker. Search for **Slash Command** and add it.
2. On the block, set **Command** to `ping`.
3. Drag from the Slash Command's green **action output** (the right side of its title) into empty canvas. The picker opens with only the blocks that fit; pick **Send Message**.
4. Connect the Slash Command's **Interaction** output to Send Message's **Interaction** input. That tells Send Message what to reply to.
5. In Send Message's **Text**, write:

   ```
   Pong! 🏓 ${Date.now() - interaction.createdTimestamp} ms
   ```

   `${…}` holds JavaScript. `interaction` is the value the Slash Command block gives; `Date.now() - interaction.createdTimestamp` is how long ago the command was used.

Your canvas now looks like this: **Slash Command** → (green wire) → **Send Message**, plus the Interaction wire.

## 6. Run it

Press **F5** (or **Run** in the title bar). The first run installs discord.js once, which takes a moment. The Console says *Logged in as Ping Bot* when the bot is online.

In Discord, type `/ping` in your server. The bot answers, and in XC Circuit the two blocks glow as they run. For half a minute, each block also shows above its corner how long it took.

::: tip When something goes wrong
A block that fails turns red and the Console says why. Common causes: no token (Bot › Settings), the bot isn't in the server, or the command hasn't been registered yet (restart the bot with **Ctrl+S** or F5).
:::

## 7. Save and stop

**Ctrl+S** saves the project (and restarts a running bot so it uses your changes). **Shift+F5** stops the bot.

## What you did

- Created a Discord application and a project for it.
- Added a slash command and two blocks: an **event** (Slash Command) that starts a flow, and an **action** (Send Message) that runs when the flow reaches it.
- Used JavaScript inside a text with `${…}`.

Next: [Values and texts](./values-and-texts) shows how blocks pass values to each other and how to store data.
