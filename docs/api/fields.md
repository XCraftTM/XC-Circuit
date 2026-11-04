# Fields and types

## Field options

```js
fields: {
  mode: { type: "select", default: "reply", pin: false, choices: { reply: "Reply", send: "Send to channel" } },
  channel: { type: "discord.channel", pin: "only", show: { mode: ["send"] }, description: "Where to send it." },
  text: { type: "textarea", default: "", description: "The message. ${…} holds JavaScript." },
  delay: { type: "number", default: 0, min: 0, step: 0.5, advanced: true, label: "Delay (s)" },
}
```

| Key | Type | Description |
| --- | --- | --- |
| `type` | text | **Required.** A [type string](#type-strings) or one of the [editors](#field-editors) below. |
| `label` | text | The name shown. Default: the key, in words (`deleteAfter` → *Delete after*). |
| `description` | text | Help shown on hover and in the Inspector. |
| `default` | any | The value of a new block. |
| `required` | boolean | Empty is a problem (listed in Problems). |
| `pin` | `"auto"` · `true` · `false` · `"only"` | `"auto"` (default): filled in on the block, with a pin toggle. `true`: starts as a pin. `false`: never a pin. `"only"`: must be connected. |
| `multi` | boolean | The pin accepts several wires; the block gets their values as one list. |
| `advanced` | boolean | Not shown on the block; the Inspector lists it under *Advanced*. |
| `show` | object | Shown only when other fields have one of these values: `{ mode: ["send", "edit"] }`. |
| `choices` | object | For `select`: value → label. A nested object makes a group: `{ "Text": { plain: "Plain", bold: "Bold" } }`. |
| `template` | boolean | Whether `${…}` in the text is filled in. Default: yes for text fields. |
| `min`, `max`, `step` | number | For numbers. |
| `placeholder` | text | Grey hint in an empty text box. |

## Field editors

Besides the [type strings](#type-strings), a field's `type` can name an editor:

| Type | Editor | Value |
| --- | --- | --- |
| `text` | a text box | text |
| `textarea` | a box of several lines, resizable | text |
| `number` | number box | number |
| `boolean` | switch | `true` / `false` |
| `select` | drop-down of `choices` | the chosen value |
| `color` | swatch with a colour picker | `"#RRGGBB"` |
| `embed` | the Embed Builder (Discord) | embed object |
| `slash-command-ref` | a slash command of the project (Discord) | its name |

## Type strings

Types say what a pin carries. The editor only connects pins whose types fit.

| Type | Values |
| --- | --- |
| `text`, `number`, `boolean`, `date` | the basic values |
| `object` | any object |
| `any` | anything; fits everywhere |
| `list<T>` | a list of `T`: `list<text>`, `list<discord.member>` |
| `T?` | a `T` or nothing: `discord.member?` |
| `A \| B` | either |
| `object<{ name: text, xp: number }>` | an object with these keys |
| `discord.user`, `fluxer.message`, … | [pin types](./pin-types) from plugins |

A type fits where its parent type is expected (`discord.member` extends `object`), and plugins can declare [conversions](./pin-types#conversions) (a member fits where a user is needed).

## define results

[`define()`](./blocks#define) returns any of these:

| Key | Description |
| --- | --- |
| `add` | More fields, by key, as field options. |
| `hide` | Keys of fields to hide. |
| `set` | Changes to existing fields: `{ text: { label: "Reason", required: true } }`. |
| `outputs` | Outputs to add or replace, by key: `{ item: { type: "discord.role" } }`. |
| `flows` | Replaces the block's `flows`. |

Example: Slash Command adds an output for each option of the chosen command.

```js
define({ values, project }) {
  const cmd = (project.data?.slashCommands || []).find((c) => c.name === values.command);
  const outputs = {};
  for (const o of cmd?.options || []) outputs[o.name] = { type: "text", description: o.description };
  return { outputs };
}
```
