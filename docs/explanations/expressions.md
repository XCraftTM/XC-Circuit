# Texts and `${…}`

Any text field (Send Message's text, Log, Join Text, the Embed and Layout Builders, …) can hold JavaScript inside `${…}`, exactly like a JavaScript template string:

```
Welcome ${member.displayName}! You're member #${server.memberCount}.
${xp >= 1000 ? "🏆 Veteran" : "🌱 Newcomer"}
It's ${new Date().getFullYear()}.
```

When the block runs, every `${…}` is evaluated and replaced by its result.

## Where names come from

A name inside `${…}` is looked up, in this order, in:

1. **The block's own fields**, by their key.
2. **The event's outputs**: `user`, `member`, `interaction`, `message`, … whatever the event that started the run gives.
3. **Run variables** set with Set Variable.
4. **Outputs of earlier blocks** on the same action path.

A name none of these give gets an **input of its own** under the text, so you can connect a value to it. Type `${ping}` and an input called *ping* appears.

## Values of a particular block

`#4` means block #4 (the number in its title bar), so `${#4.message.url}` is the *Message* output of block #4. Use this when several earlier blocks give an output with the same name, or when you want to be explicit. Typing `${` in a text lists the blocks before this one and their values; the editor writes the reference for you.

## Members of values

Values are real objects: a Discord *User* is a discord.js `User`, a *Member* a `GuildMember`. Their properties and reading methods work: `${user.username}`, `${member.roles.highest.name}`, `${user.displayAvatarURL()}`. Typing a `.` after a value inside `${…}` lists its members with their types, like a code editor.

## Errors

A mistake in an expression (a typo, reading a property of nothing) stops the block with an error that quotes the expression. Use optional chaining and fallbacks where a value may be missing: `${member?.nickname ?? user.username}`.

## Why JavaScript

Template strings are the JavaScript most people already know, they work for everything from a name to a calculation, and they run exactly as written: there's no separate formula language to learn, and anything you find in discord.js's documentation can be used directly.

::: details Older projects
Projects from before `${…}` used `{name}`. They are changed to `${name}` when the project opens.
:::
