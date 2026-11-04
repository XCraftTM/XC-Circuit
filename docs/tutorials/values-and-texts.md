# Values and texts

A `/count` command that remembers how often each person used it: **"You've counted 3 times, Alex!"**

Start from the bot of [Your first bot](./first-bot).

## 1. Add the command

- **Bot › Slash Commands…** › **+ New command**: `count`.
- In the Workspaces panel, **+** adds a workspace: call it *Count command*.

## 2. Build it

Add **Slash Command**, **Increase Stored Value** and **Send Message**, connected with green arrows, then:

![The Count command's blocks](/screens/count-blocks.png)

1. Connect **User** to **Target**: every person gets their own count.
2. Set **Key** to `counts`.
3. Connect **Value** to the text's `${value}` input. It appears by itself when you type `${value}` in the text.
4. The text:

```
You've counted ${value} times, ${user.username}!
```

Also connect **Interaction** to Send Message's **Interaction**, as before.

## 3. Try it

**F5**, then use `/count` a few times. The number keeps going up, also after restarting the bot. The **Stored Data** view (database icon on the left) shows what's saved.

::: tip Let the editor write it
Type `${` in any text: a list shows what earlier blocks give. Pick one and it's written for you, such as `${#2.value}`.
:::

## What's what

| | |
| --- | --- |
| **Outputs** (right side of a block) | values a block gives |
| **Inputs** (left side) | values a block takes |
| **`${…}`** in a text | JavaScript; names from earlier blocks just work |
| **Stored values** | kept across restarts, per user, server, member, channel or bot |

More: [Texts and ${…}](/explanations/expressions) · [Variables and stored data](/explanations/data).
