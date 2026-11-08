# Transcripts

::: info In short
**Create Transcript** saves a channel's messages as one HTML file that looks like Discord: for a ticket before it's closed, or a channel you archive. Send it with Send Message's **Files**, or keep it with **Write File**.
:::

## Using it

A ticket's *Close* button, for example: On Button › **Create Transcript** (the ticket's channel) › Send Message to a `#ticket-logs` channel with the transcript in **Files** › Delete Channel.

| Setting | What it does |
| --- | --- |
| **At most** | The latest messages to keep (1000). 0 keeps all; in a long channel that takes a while. |
| **File name** | `transcript-${channel.name}.html` by default. |
| **Keep pictures** | Puts pictures into the file, so they still show when Discord's links expire. The file gets much bigger. |
| **Bots' messages too** | Off leaves out what bots wrote, such as the ticket's own buttons. |

It gives the **file**, how many messages it holds (**count**) and its **size**.

The file shows messages as Discord does: names and avatars, replies, embeds, Components V2 layouts, attachments, polls, threads. It opens in any browser.

## Safety

The bot needs **Read Message History** in the channel, and the *Message Content* intent (Bot › Settings › Privileged intents).

The transcript is drawn completely on the bot, so the file loads no scripts from the web, and what people wrote (names, messages) can't put code into it.

It uses [better-discord-transcripts](https://www.npmjs.com/package/better-discord-transcripts) 2.0.0, a version we read through before using it (no install scripts, nothing sent anywhere). It's installed with the bot the first time the block runs (with its helpers, about 80 MB).
