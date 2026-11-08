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

Values are real objects: a Discord *User* is a discord.js `User`, a *Member* a `GuildMember`. Their properties and reading methods work: `${user.username}`, `${member.roles.highest.name}`, `${user.displayAvatarURL()}`. Typing a `.` after a value inside `${…}` lists its members with their types, like a code editor. Collections (a server's `members.cache`, a member's `roles.cache`) list their own: `size`, `first()`, `get(id)`, `find(…)`, `filter(…)` …, and after one that gives back an item, the item's: `${server.members.cache.get(user.id).displayName}`, `${member.roles.cache.first().name}`. A Map lists `size`, `get(…)` and `has(…)`.

## Mentions

A user, member, role or channel in a text is a mention: `Welcome ${user}!` pings them, `${role}` pings the role and `${channel}` links the channel, as `<@id>`, `<@&id>` and `<#id>` do. For a name without a ping, use `${user.username}`, `${member.displayName}` or `${role.name}`. The list after `${` says which values ping (**pings them**) or link (**links it**). The **Mention** block does the same for an id, a list of people, `@everyone` or `@here`; Send Message's **Who gets pinged** decides who is notified.

## Splitting text

**Split Text** takes a text apart at a separator (spaces when it's empty, or `,`, `|` …) and gives the parts ready to use: **First**, **Last**, **Part** (by number: 1 is the first, -1 the last), **Rest** (everything after the first part) and **Count**. For `!kick @ada being rude`: First is `!kick`, Part 2 is `@ada`, Rest is `@ada being rude`.

## Dates and times

- **Date & Time**: now (or a given date), moved with **Add** (`1d 2h`, `30m`, `-1w`), as text: your own pattern such as `DD.MM.YYYY HH:mm` in a time zone such as `Europe/Berlin`, or a Discord timestamp that each reader sees in their own time zone and language.
- **Read Date**: what people type, as a date: `2026-12-24 18:00`, `24.12.2026`, `18:30`, `tomorrow 9:00`, `in 2h`, `1d 12h`, a unix time or a Discord timestamp. **Valid** says whether it could be read.
- **Time Between**: the time between two dates (empty is now) in days, hours, minutes and seconds, and as text such as `2 d 3 h`. A user or member counts from when the account was made.

## Errors

A mistake in an expression (a typo, reading a property of nothing) stops the block with an error that quotes the expression. Use optional chaining and fallbacks where a value may be missing: `${member?.nickname ?? user.username}`.

## Why JavaScript

Template strings are the JavaScript most people already know, they work for everything from a name to a calculation, and they run exactly as written: there's no separate formula language to learn, and anything you find in discord.js's documentation can be used directly.

::: details Older projects
Projects from before `${…}` used `{name}`. They are changed to `${name}` when the project opens.
:::
