# Pictures with Build Canvas

::: info In short
**Build Canvas** draws a picture (a welcome card, a rank card, a profile) from a design you make in the **Canvas Builder**. Connect its **File** to Send Message's **Files**.
:::

## The Canvas Builder

Add **Build Canvas** (in *Embeds & Layouts*) and press **Edit canvas…** on it, or **Open Canvas Builder** in the Inspector.

- **Start from** a Rank card, Welcome card or Profile card, or start empty.
- **Size** in pixels, with common sizes to pick from, and a **background** colour (or see-through).
- **Layers**, drawn bottom to top:

| Layer | What it is |
| --- | --- |
| **Shape** | A rectangle, with round corners if you like. |
| **Circle** | A circle or an ellipse. |
| **Picture** | From a web address, a file, or `${…}` such as a user's avatar. Square, round corners or a circle; *Fill* cuts off what doesn't fit, *Whole picture* shows all of it. |
| **Text** | In Regular, Bold or Monospace; left, centred or right in its width. Long texts wrap; *Shrink to fit* makes one line smaller until it fits (good for names). |
| **Bar** | A progress bar: *Value* out of *Out of*, such as `${xp}` out of `${needed}`. |

Click a layer in the picture (or the list) to choose it, drag it to move it, drag its corner to make it bigger or smaller (hold Shift to keep its shape). The arrow keys move it by a pixel, with Shift by ten. On the right are its settings: position, size, opacity, colour, a **gradient** to a second colour, an **outline**, and the rest of what its kind has.

The preview uses the same fonts as the bot, so what you see is what the bot draws, with two differences: pictures only show where they go (the bot loads them when it runs), and `${…}` shows its **preview value**.

### `${…}` in a design

Texts, pictures, bar values and colours can hold `${…}`: `${user.username}`, `${level}`, `${role.hexColor}`. As in other texts, a name that no earlier block gives gets an input on the block to connect.

Under the picture, **Preview values** has a box for each `${…}`: type what to show in the preview (`Alex` for `${user.username}`). They're only for the preview.

A user's avatar: `${user.displayAvatarURL({ extension: 'png', size: 256 })}`.

## Sending the picture

- Connect **File** to Send Message's **Files** (in the Inspector, under Advanced).
- To show it inside an embed, put the block's **URL** (`attachment://canvas.png`) into the embed's image, and send the file with the message too.

**File name** (under Advanced) changes `canvas.png`.

A picture that can't be loaded (a wrong address, say) is left out, with a warning in the Console; the rest is drawn.

## Under the hood

The bot draws with [@napi-rs/canvas](https://www.npmjs.com/package/@napi-rs/canvas), installed with the bot's other packages. The fonts (Ubuntu, Ubuntu Bold, Hack and Noto Emoji) come with the block, so exported bots draw the same on any server.

Fluxer bots can build pictures too, but Fluxer's Send Message can't send files yet.
