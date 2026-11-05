// The permissions blocks say they need (`requires.permissions`), and the
// plain message a "Missing Permissions" error becomes.
// Usage: node runtime/test/permissions.js
"use strict";

const fs = require("fs");
const path = require("path");
const assert = require("assert");
const { ungroupBlock } = require("../circuit-runtime.js");
const discord = require("../../plugins/xc.discord/runtime/discord.js");

// Every annotation names permissions the way Discord's flags do, and a list
// per value points at a real field and real choices.
let annotated = 0;
for (const plugin of ["xc.discord", "xc.fluxer"]) {
  const dir = path.join(__dirname, "..", "..", "plugins", plugin, "blocks");
  for (const group of fs.readdirSync(dir)) {
    for (const f of fs.readdirSync(path.join(dir, group))) {
      const block = ungroupBlock(require(path.join(dir, group, f)));
      const p = block.permissions;
      if (p === undefined) continue;
      annotated++;
      const lists = Array.isArray(p) ? [p] : [];
      if (!Array.isArray(p)) {
        assert.ok(p && typeof p === "object", `${block.id}: permissions is a list or { field: { value: list } }`);
        for (const [field, byValue] of Object.entries(p)) {
          const choices = block.fields?.[field]?.choices;
          assert.ok(choices, `${block.id}: permissions by ${field}, which is no field with choices`);
          for (const [value, list] of Object.entries(byValue)) {
            assert.ok(value in choices, `${block.id}: ${field} has no choice ${value}`);
            lists.push(list);
          }
        }
      }
      for (const list of lists) {
        assert.ok(Array.isArray(list) && list.length, `${block.id}: a list of permissions`);
        for (const name of list) assert.match(name, /^[A-Z][A-Za-z]+$/, `${block.id}: ${name} looks like a permission flag`);
      }
    }
  }
}
assert.ok(annotated >= 50, `blocks say which permissions they need (${annotated})`);

// The message for Missing Permissions names what this block needs.
const moderate = ungroupBlock(require("../../plugins/xc.discord/blocks/members/moderate.js"));
assert.deepStrictEqual(discord.neededPermissions(moderate, { action: "kick" }), ["KickMembers"]);
assert.deepStrictEqual(discord.neededPermissions(moderate, { action: "nope" }), []);
const kick = discord.explainError({ code: 50013, message: "Missing Permissions" }, moderate, { action: "timeout" });
assert.match(kick, /needs this: Timeout Members\./);
const webhooks = ungroupBlock(require("../../plugins/xc.discord/blocks/channels/webhooks.js"));
assert.match(discord.explainError({ code: 50013 }, webhooks, {}), /Manage Webhooks/);
const lock = ungroupBlock(require("../../plugins/xc.discord/blocks/channels/lock_channel.js"));
assert.match(discord.explainError({ code: 50013 }, lock, {}), /needs these: Manage Channels, Manage Roles\. Give its role them/);
assert.match(discord.explainError({ code: 50001 }, lock, {}), /can't see this channel/);
assert.strictEqual(discord.explainError({ code: 10008, message: "Unknown Message" }, lock, {}), null, "other errors stay as they are");

console.log(`${annotated} blocks with permissions checked`);
console.log("permissions test passed");
