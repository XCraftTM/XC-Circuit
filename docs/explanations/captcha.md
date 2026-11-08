# Captcha

::: info In short
**Captcha** checks that someone is a person before they get in. It ends on **Passed**, **Failed** or **Timed Out**; connect Passed to Add Role, say.
:::

## Discord

A typical verification: a message with a **Verify** button in a `#verify` channel, an **On Button** for it, then **Captcha** with the button's **interaction**. The challenge appears in a message only that person sees.

| Mode | They… |
| --- | --- |
| **Type the code from a picture** | press *Answer* and type the code shown in a picture (letters that can't be mixed up; upper or lower case doesn't matter) |
| **Solve a sum** | press *Answer* and type the result, such as 7 × 6 |
| **Click the named emoji** | click the one of five emoji the message names (*Click the pizza*) |
| **Just click a button** | click *I'm a person*: the easiest, it stops only simple bots |

- **Tries** (3) and **Time** (3 minutes, in all). After a wrong answer they get a new challenge.
- **Where**: to the interaction (from a button or a slash command; only they see it), or as a **direct message** (from On Member Join, say; people can turn DMs off, so a button is more reliable).
- **Code length** for the picture: 4 to 8.

Then, for example: Passed › Add Role *Verified*; Failed › Moderate *kick*; Timed Out › Moderate *kick*, or nothing.

## Fluxer

Fluxer has no buttons yet, so its Captcha asks in a **channel**, mentioning the person, and waits for their answer there: the code from a picture, or a sum. *Clean up* (on) deletes the questions and answers afterwards.

## The picture

The bot draws it with the same package as [Build Canvas](./canvas) and brings its own font, so it works on any server.
