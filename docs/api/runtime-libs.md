# Runtime libraries

A runtime library is code a build mode runs when the bot starts: it creates the client (discord.js, fluxer.js), logs in, and gives the mode's blocks a shared API. A mode names its libraries in [`runtimeLibs`](./build-modes#keys).

```js
// plugins/acme.chat/runtime/chat.js
module.exports = {
  id: "chat",                         // blocks get the API as ctx.chat

  setup(rt) {
    const { Client } = rt.require("acme-chat");
    const client = new Client();
    client.on("error", (e) => rt.log.error(`Chat: ${e.message}`));
    return { client, prefix: () => rt.setting("prefix") || "!" };
  },

  async start(rt, api) {
    const token = rt.secret("token");
    if (!token) throw new Error("No bot token set. Add it in Bot › Settings.");
    api.client.once("ready", () => rt.log.success(`Logged in as ${api.client.user.name}`));
    await api.client.login(token);
  },
};
```

## When they run

1. **`setup(rt)`** for each library, in order. What it returns becomes `ctx[id]` for every block.
2. Blocks' own `setup(ctx)`.
3. Event blocks' [`listen(ctx, emit)`](./blocks#listen): they subscribe to the client now, before it connects.
4. **`start(rt, api)`** for each library: connect and log in. A thrown error is shown in the Console.
5. [`ctx.runtime.onStart`](./ctx#runtime) handlers.

## rt

| Member | Description |
| --- | --- |
| `rt.require(name)` | Loads an npm package (list it in the mode's [`[dependencies]`](./build-modes#dependencies)). |
| `rt.setting(key)` | A setting of Bot › Settings. |
| `rt.secret(key)` | A secret setting: from the credential store when run from the editor, from `.env` in an exported bot. `rt.secret("token")` is the bot token. |
| `rt.log.info/success/warn/error/debug(...)` | Writes to the Console. |
| `rt.intents()` | The Discord intents the blocks in the project need (their `requires.intents`). |

Look at `plugins/xc.discord/runtime/discord.js` and `plugins/xc.fluxer/runtime/fluxer.js` for complete libraries.
