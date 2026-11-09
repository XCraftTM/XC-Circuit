# Fields and types

::: info In short
A block's `fields` are what people fill in on it (or connect a wire to); each has a `type` that decides its editor and which wires fit.
:::

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
| `pin` | `"auto"` · `true` · `false` · `"only"` | `"auto"` (default): filled in on the block, with a pin toggle. `true`: starts as a pin. `false`: never a pin. `"only"`: must be connected (inputs whose pin type has [`fromId`](./pin-types#finding-values-by-id), such as channels, can still be switched to a box for an id). |
| `multi` | boolean | The pin accepts several wires; the block gets their values as one list. |
| `advanced` | boolean | An option used less often. On the block it goes below **More options**, folded to start with: folded, it shows only when it's in use (a pin, connected, or set to something other than its default). The Inspector lists it under *More options*; its **pin** chip (also for `pin: "only"` fields) puts its pin on the block. |
| `show` | object | Shown only when other fields have one of these values: `{ mode: ["send", "edit"] }`. |
| `choices` | object | For `select`: value → label. A nested object makes a group: `{ "Text": { plain: "Plain", bold: "Bold" } }`. |
| `template` | boolean | Whether `${…}` in the text is filled in. Default: yes for text fields and colours. |
| `templateParts` | boolean | Instead of filling `${…}` in, give the block `{ strings, values }`: the text around them and their values, as a JavaScript tagged template gets them. SQL Query uses it to send the values as parameters. |
| `min`, `max`, `step` | number | For numbers. |
| `placeholder` | text | Grey hint in an empty text box. |
| `custom` | boolean | For a field with `choices`: the list ends in **Custom…**, which shows a text box for any other value (and so does a value that isn't one of the choices). The value stays one text, so a `text` field like this can still be connected. Get Property uses it for the properties of the connected value. |
| `suggest` | `"button"`, `"select"` or `"form"`: a custom id field of an event. It offers the ids of that kind the project gives (in layouts, forms and fields with `customIdOf`), with Custom… for others, and Problems checks the two match. |
| `customIdOf` | `"button"`, `"select"` or `"form"`: this field gives a button, menu or form its custom id (Button, Select Menu). |
| `pick` | `"channel"`, `"voice"`, `"category"` or `"role"`: a text field for an id (or name) that gets a button listing the bot's servers' channels or roles; picking one puts its id in. Fields for channels by id get it from their type. |

## Field editors

Besides the [type strings](#type-strings), a field's `type` can name an editor:

| Type | Editor | Value |
| --- | --- | --- |
| `text` | a text box | text |
| `textarea` | a box of several lines, resizable | text |
| `number` | number box | number |
| `boolean` | switch | `true` / `false` |
| `select` | drop-down of `choices` | the chosen value |
| `multiselect` | drop-down of `choices` with a checkbox each (and a search when there are many) | a list of the chosen keys; text with commas from older projects is read too |
| `color` | swatch with a colour picker, or (with its `${}` button) `${…}` text for a colour from the bot; like other fields it can also be a pin | `"#RRGGBB"`; from a pin or `${…}`, any hex colour with or without `#` (`#FF5CA8`, `FF5CA8`, `#F5A`), which the block reads as a colour |
| `embed` | the Embed Builder (Discord) | embed object |
| `layout` | the Layout Builder, and the message on the block (Discord) | Components V2: `{ components: [ … ] }` |
| `modal` | the Modal Builder, and the form on the block (Discord) | Discord's modal JSON: `{ title, custom_id, components: [ … ] }`, with questions (type 18 around a text input, select menu or file upload) and texts (type 10) |
| `slash-command-ref` | a slash command of the project (Discord) | its name |
| `canvas` | the Canvas Builder, and the picture on the block | a design: `{ width, height, background, layers, samples }` ([Build Canvas](/explanations/canvas)) |

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
| `add` | More fields, by key, as field options. They go after the block's own fields, unless a field says `after`: the key of the field it follows, or `""` for the very top (Send Message puts what it sends to there). |
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
