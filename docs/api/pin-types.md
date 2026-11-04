# Pin types

::: info In short
Types of values on wires (`discord.member`, `fluxer.message`…): their name, colour, how to check a value, and what they convert to.
:::

Pin types are the types of values that go through wires besides the basic ones (`text`, `number`, `boolean`, `date`, `object`, `list<…>`, `any`). They're TOML files listed in a plugin's `contributes.pinTypes`, each with one `[[type]]` table per type.

```toml
[[type]]
id = "discord.member"
name = "Member"
extends = "object"
color = "#6C7BFF"
icon = "user"
check = "v => v?.constructor?.name === 'GuildMember'"
description = "A person inside one server, with roles and a nickname."

[[type.converts]]
to = "discord.user"
via = "v => v.user"
```

## Keys

| Key | Description |
| --- | --- |
| `id` | **Required.** `plugin.name`, used in [type strings](./fields#type-strings). |
| `name` | Shown on pins and in tooltips. |
| `extends` | The type this one fits into, usually `object`. A `discord.member` pin can connect where an `object` is expected. |
| `color` | The colour of its pins and wires. Themes can change it. |
| `icon` | An icon name. |
| `check` | A JavaScript function (as text) the runtime uses to check that a value really is of this type. |
| `description` | What the type is, for tooltips. |

## Conversions

`[[type.converts]]` lets a value of this type go where another type is needed. `via` is a JavaScript function (as text) that turns the value into the other type when the block runs.

```toml
[[type.converts]]
to = "discord.user"
via = "v => v.user"
```

Now a Member output connects to a User input, and the block gets the member's user.

## Class catalogs

A class catalog tells the editor what members the values behind pin types have, for completion inside `${…}` (`${member.roles.highest.name}`). It's a JSON file listed in `contributes.classes`:

```json
{
  "source": "discord.js 14.28.0",
  "roots": { "discord.user": "User", "discord.member": "GuildMember" },
  "classes": {
    "User": {
      "members": [
        { "name": "username", "kind": "property", "type": "Text" },
        { "name": "avatar", "kind": "property", "type": "Text", "optional": true },
        { "name": "displayAvatarURL", "kind": "method", "type": "Text", "doc": "The avatar's address." }
      ]
    }
  }
}
```

| Key | Description |
| --- | --- |
| `source` | What it was made from. |
| `roots` | The class behind each pin type. |
| `classes` | Classes by name, each with `members`. |
| member `name` | The property or method name. |
| member `kind` | `property`, `getter` or `method`. |
| member `type` | How its type is shown: `Text`, `Number`, `Yes/no`, `Date` or a class name. |
| member `optional` | It can be empty. |
| member `class` | The class of its value, when that has members of its own (completion goes on after the next `.`). |
| member `doc` | A short description. |

The Discord plugin makes its catalog from discord.js's type definitions with `node plugins/xc.discord/tools/discordjs-catalog.js`; only methods that read are listed.
