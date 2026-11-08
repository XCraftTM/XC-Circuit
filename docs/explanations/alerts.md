# New-post alerts

::: info In short
**On New Post** runs once for each new video, post or release of a source you choose, and when a Twitch or Kick channel goes live. Connect it to Send Message to announce it.
:::

## Sources

| Source | What to write in *Channel* | Needs |
| --- | --- | --- |
| **YouTube channel** | `@handle`, the channel's link, or its id (`UC…`) | nothing. *Skip Shorts* leaves Shorts out. |
| **Twitch** (goes live) | the channel name, or its link | a client id and secret: Bot › Settings › New-post alerts |
| **Kick** (goes live) | the channel name, or its link | a client id and secret: Bot › Settings › New-post alerts |
| **Reddit** | the subreddit: `r/name` or `name` | nothing |
| **Bluesky** | the handle: `name.bsky.social` | nothing |
| **Mastodon** | `@name@server`, or the profile's link | nothing |
| **GitHub releases** | the repository: `owner/name` | nothing |
| **Steam news** | the game's app id (the number in its store link), or the link | nothing |
| **RSS or Atom feed** | the feed's address, under *Feed address* | nothing. Most blogs, news sites and podcasts have one. |

### Twitch and Kick keys

Both want an application of your own, which takes a minute and costs nothing:

- **Twitch:** at [dev.twitch.tv](https://dev.twitch.tv/console/apps) › Your Console › Applications › Register, with any name, the category *Chat Bot* and `http://localhost` as OAuth Redirect URL. Copy the **Client ID**, press **New Secret** and copy that too.
- **Kick:** at kick.com › Settings › Developer › Create app, with `http://localhost` as Redirect URL. Copy the **Client ID** and **Client Secret**.

Paste them into **Bot › Settings › New-post alerts**. The secrets are kept in the system's password store.

## What comes out

**Title**, **URL**, **Text** (without formatting, shortened), **Author**, **Image** (a thumbnail, when there is one) and **Date**. **Post** has all of it, with what the source has besides: `post.game` and `post.viewers` for streams, `post.nsfw` for Reddit, `post.short` for YouTube.

Announcing a new video in an embed: On New Post › Build Embed (title `${title}`, link `${url}`, image `${image}`) › Send Message to your channel.

## How it checks

- It looks every **Check every** minutes (5 by default; at least 1).
- **The first time**, posts that are already there only count as seen, so a new bot doesn't announce a channel's last 15 videos. Turn on *Announce what's there at first* to announce the newest few.
- **Nothing twice:** what it has seen is kept with the stored data, so a restart doesn't announce old posts again. More than 5 new posts at once (a feed that was off for a while): the 5 newest are announced.
- **Only if it contains** announces only posts whose title or text has a word or phrase.
- When a source can't be reached, the Console says so once and it keeps trying; it says when it works again.

Two On New Post blocks for the same channel each announce on their own (to two servers, say).
