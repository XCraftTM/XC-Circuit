# Try Zones and errors

Things go wrong in bots: Discord says no, a member left a moment ago, a website is down. XC™ Circuit makes failures visible and gives you a way to deal with them.

## What happens when a block fails

When a block throws an error, its run stops there. The block turns red on the canvas, the Console says which block failed and why, and the other runs of the bot carry on. A failed run doesn't stop the bot.

If you have an **On Error** event, it runs when a block outside any Try Zone fails, with the message, the block's name and details: use it to log errors to a channel, for example.

## Try Zones

A Try Zone is a frame around some blocks that catches what goes wrong inside it. Select a block and choose **Try Zone** above it; drop more blocks in or out.

A zone has its own outputs:

- **Done**: the flow continues here when everything inside finished.
- **Error**: the flow continues here when something inside failed, with the error's **message** (and details) as outputs.

And two settings (the gear on the zone's title):

- **Retries**: try the blocks inside again this many times before giving up. Good for things that fail now and then, such as web requests.
- **Timeout**: give up when the blocks inside take longer than this.

If nothing is connected to **Error**, the zone logs a warning and the run ends there quietly: it doesn't count as a failure, and On Error doesn't run.

## Problems before running

The Problems panel lists what the editor can see before the bot runs, such as required fields that are empty or blocks from plugins that aren't installed. Click an entry to go to the block. Plugins can add their own [checks](/api/editor#xc-check).

## In your own blocks

A block fails by throwing: `throw new Error("No channel with that id")`. Write messages for the person building the bot: say what's wrong and, if you can, what to do about it.
