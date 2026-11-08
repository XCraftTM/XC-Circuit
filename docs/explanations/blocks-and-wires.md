# Events, actions and values

A bot in XC™ Circuit is a graph of blocks connected by wires. Every block is one of three kinds, and the kind decides when it runs.

## The three kinds

| Kind | Chip | Runs… | Examples |
| --- | --- | --- | --- |
| **Event** | Event | when something happens: a command is used, a member joins, a timer fires | Slash Command, Member Joins, Every, On Start |
| **Action** | Action | when the flow reaches it, one after the other | Send Message, Add Role, If, Wait |
| **Value** | Value | where the flow reaches it, or when a block needs one of its outputs | Math, Join Text, Get Property |

An event **starts a flow**. The flow follows the green **action wires** from block to block: each action runs, finishes, and passes the flow on. Value blocks can go **in the flow** too: wire an Action into them and on out of them, and they run right there, once, and every block after them gets that result (a Random Number in the flow gives two later blocks the same number). That's the clearest way to read, say, a property of a command's option before using it. Left **to the side**, without Action wires, they're worked out when a block that's about to run needs their output, afresh each time.

```
[Slash Command] ──▶ [Get Stored Value]* ──▶ [Send Message]
                              ▲
                     [Math] ──┘  (values are worked out on demand)
```

## Two kinds of wires

- **Action wires** (green) connect an action output to an action input. They decide the order.
- **Value wires** connect an output, such as *User* or *Value*, to an input of the same or a compatible type. They carry data.

A value can come from any block that ran earlier in the same flow, or from a value block. The editor only lets you connect pins whose types fit (a *Member* can go where a *User* is needed, because a member converts to its user), and dragging a wire into empty canvas offers only the blocks that fit.

## One path per action output

An action output leads to one block; connecting it elsewhere replaces the old wire. Paths can join, but they only **branch** through blocks made for it: **If** (true/false), **Switch**, **For Each**, **Repeat**, or a block of your own with several [`flows`](/api/blocks#flows). This keeps the order of a flow obvious: you can always read it from left to right. A wire that would make an endless loop is refused.

## Several flows at once

Every event starts its own **run**. If two people use a command at the same time, two runs go through the same blocks independently, each with its own values and run variables. Blocks that wait (for Discord, a timer, the web) don't hold up other runs.

## Fields and pins

A block's **fields** are what you fill in on it: a text, a number, a choice. Most fields can also become a **pin**: click the faint pin next to the field (or *pin* in the Inspector) and connect a value to it instead. Some fields are pins only, because a fixed value makes no sense there (a member to give a role to, for example).

## Inputs the event fills in

An **Interaction** or **Server** input left unconnected takes the one of the event that started the flow (Slash Command, Button Pressed, Member Joined …), when that event gives one: the block says **from the event** instead of *connect*, and it isn't a problem. So Send Message replying to a command needs no wire for the interaction. Users, members, channels and messages are never filled in like this: an unconnected Ban or Delete Channel must not hit whoever started it, so connect those.

## Why it's built this way

- **Order you can see.** Bots are mostly "when X, do A, then B, then C". Action wires make that order visible and keep it unambiguous.
- **Values without wiring order.** Calculations don't need to be placed in the flow; they're worked out when used, so the flow stays short.
- **Plain JavaScript underneath.** Every block is a [JavaScript file](/api/blocks), and every text accepts [`${…}`](./expressions), so you can always drop down to code when a block doesn't exist yet.
