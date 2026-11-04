# Values and texts

In this tutorial you extend the bot from [Your first bot](./first-bot) with a `/count` command that counts how often each person used it, and answers "You've counted 3 times, Alex!". On the way you use a value from one block in another, write texts with `${…}`, and store data that survives restarts.

## 1. A new workspace

Workspaces keep a project tidy: one per command or feature is a good habit. In the Workspaces panel, choose **+** and name the new workspace *Count command*.

Add the `count` command in **Bot › Slash Commands…** as you did for `ping`.

## 2. Count, per person

1. Add a **Slash Command** block and set its command to `count`.
2. Drag from its action output into empty canvas and add **Increase Stored Value**.
3. On Increase Stored Value, keep **Scope** on *User* and **Key** on `xp`, but change the key to `counts`. Connect the Slash Command's **User** output to its **Target** input.

Increase Stored Value adds 1 to a number stored for that user, starting from 0, and gives the new number as its **Value** output.

## 3. Answer with the number

1. Drag from Increase Stored Value's action output and add **Send Message**.
2. Connect the Slash Command's **Interaction** to Send Message's **Interaction**.
3. Write the text:

   ```
   You've counted ${value} times, ${user.username}!
   ```

Look at the block after typing: `value` got an input of its own below the text, because nothing earlier on the path gives a value called `value`. Connect Increase Stored Value's **Value** output to it. `user` needs no input: the Slash Command block, earlier on the path, already gives it.

::: tip Let the editor write it
Type `${` in the text and a list opens with the blocks before this one and their values. Pick **#2** and then **Value**, and the editor writes `${#2.value}`: block #2's Value output, with no extra input needed.
:::

## 4. Try it

Press **F5** and use `/count` a few times. The number goes up, also after stopping and starting the bot again: stored values are kept in a file (`.circuit/data/store.json` in the project folder). Open the **Stored Data** view in the side bar (the database icon) to see them.

## What you learned

- **Outputs** carry values from one block to the next. A value is worked out when an action needs it.
- In texts, `${…}` is JavaScript. Names come from the event, earlier blocks and the block's own fields; any other name gets an input to connect.
- **Stored values** persist; per user, server, member, channel or for the whole bot.

Read more in [Texts and ${…}](/explanations/expressions) and [Variables and stored data](/explanations/data).
