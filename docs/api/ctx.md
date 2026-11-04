# Run context (ctx)

::: info In short
Inside a block's `run()`, `ctx` is the running bot: `ctx.next()` to continue a flow, `ctx.log`, variables (`ctx.vars`), saved data (`ctx.store`), the Discord client (`ctx.discord.client`) and more.
:::

Blocks get `ctx` in [`run({ fields, ctx, node })`](./blocks#run), [`listen(ctx, emit)`](./blocks#listen) and `setup(ctx)`. It's how a block reaches the running bot: logging, variables, stored data, signals, and the libraries of the build mode.

This runs in the **bot** (Node.js), not in the editor.

## next

```js
await ctx.next(flow, outputs?)
```

Continues the flow on one of the block's [`flows`](./blocks#flows), after setting `outputs`. Resolves when everything after it has finished, so a loop can call it once per item. Only actions with `flows` call it.

## set

```js
ctx.set("progress", 50)
```

Sets one output without continuing. Useful in loops, or before a `next` that doesn't pass outputs.

## fields

The block's own field values as filled in (without connected values). In `run`, prefer the `fields` argument, which includes connected values.

## event

The outputs of the event that started this run: `ctx.event.user`, `ctx.event.message`… Undefined in `listen` and `setup`.

## log

```js
ctx.log.info("Synced", count, "roles");
ctx.log.success("Done");
ctx.log.warn("Slow answer");
ctx.log.error("Could not reach the API");
ctx.log.debug({ payload });
```

Writes to the Console (and, in exported bots, the terminal), naming the block. Objects are shown readably.

## vars

Variables in memory, gone when the bot stops. See [Variables and stored data](/explanations/data).

```js
ctx.vars.run.set("total", 3);           // this run only
ctx.vars.workspace.get("queue", []);    // shared by this workspace's runs
ctx.vars.global.has("ready");           // shared by the whole bot
```

Each has `get(name, fallback?)`, `set(name, value)`, `has(name)` and `delete(name)`.

## store

Data kept in `store.json`, which survives restarts.

```js
const xp = await ctx.store.member(server.id, user.id).inc("xp", 5);
await ctx.store.global().set("lastRestart", Date.now());
```

| Scope | Values per |
| --- | --- |
| `ctx.store.global()` | bot |
| `ctx.store.server(id)` | server |
| `ctx.store.user(id)` | user |
| `ctx.store.member(serverId, userId)` | member of a server |
| `ctx.store.channel(id)` | channel |
| `ctx.store.ns(name)` | your own namespace: keep a plugin's data apart |

Each scope has (all return promises):

| Method | Description |
| --- | --- |
| `get(key, fallback?)` | The value, or `fallback`. |
| `set(key, value)` | Stores a value (anything JSON can hold). |
| `has(key)` | Whether there's a value. |
| `delete(key)` | Removes it. |
| `inc(key, by = 1)` | Adds to a number (from 0) and gives the new number. |
| `push(key, value)` | Appends to a list (from empty) and gives the list. |
| `keys()` | Every key in this scope. |

`ctx.store.each(scope, serverId?)` lists every stored entity of a scope: `[{ id, values }]`, for leaderboards. Writes are saved to disk within a quarter of a second.

## signal

```js
ctx.signal.emit("level-up", { user, level });
```

Starts every **On Signal** event with that name, without waiting for them.

## runtime

| Member | Description |
| --- | --- |
| `onStart(fn)` | Runs `fn` once the bot has started and is connected. |
| `onSignal(name, fn)` | Runs `fn(data)` for each signal with that name. |
| `onError(fn)` | Runs `fn(error)` for errors outside any Try Zone. |
| `onCall(name, fn)` | Answers Call blocks with that name: `fn(data)` starts a run and resolves to what it returns. |
| `call(name, data)` | Runs the When Called flows with that name and resolves to the first one's return value. |
| `setting(name)` | A setting of Bot › Settings (not a secret). |

## returnValue

```js
ctx.returnValue(value)
```

Sets what this run gives back to the Call block (or `emit`) that started it. The Return block uses this.

## resolve

```js
const fresh = await ctx.resolve();
```

Reads the block's inputs again. Loops whose condition changes while they run (Loop While) use it.

## project

`{ name, mode, settings, data }`: the project's name, build mode, settings (without secrets) and data, such as `data.slashCommands`.

## fill

```js
ctx.fill("Hello ${name}!", { name: "Alex" })
```

Fills `${…}` in a text the way text fields are filled: from the event, run variables, earlier outputs and the names you pass.

## util

| Member | Description |
| --- | --- |
| `sleep(ms)` | A promise that resolves after `ms` milliseconds. |
| `isURL(text)` | Whether the text is a web address. |
| `id(length = 10)` | A random id of letters and digits. |
| `shuffle(list)` | A shuffled copy. |
| `typeOf(value)` | `"null"`, `"list"`, or JavaScript's `typeof`. |

## bot

| Member | Description |
| --- | --- |
| `uptime` | Milliseconds since the bot started. |
| `stop()` | Stops the bot. |
| `restart()` | Restarts the bot. |

## abortSignal

Inside a [Try Zone](/explanations/errors) with a timeout: an `AbortSignal` that fires when the zone gives up. Pass it to `fetch` and other APIs that accept one, so long work stops in time.

## require

```js
const fetch = ctx.require("node-fetch");
```

Loads an npm package. List it in the block's [`requires.dependencies`](./blocks#requires) so it's installed before Run and included in exports. Node's own modules (`node:crypto`, `node:fs`) work too.

## Mode libraries

[Runtime libraries](./runtime-libs) add their API under their id:

| Mode | Member | What it is |
| --- | --- | --- |
| Discord Bot | `ctx.discord.client` | the discord.js `Client` |
| Fluxer Bot | `ctx.fluxer.client`, `ctx.fluxer.prefix()`, `ctx.fluxer.channel(v)`, `ctx.fluxer.user(v)`, `ctx.fluxer.member(v, server)`… | the fluxer.js client and helpers that accept objects or ids |
