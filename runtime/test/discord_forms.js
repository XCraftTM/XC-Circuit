// Forms (modals) with Components V2: Form Field, File Upload, Form Text and
// menus in Show Modal, and what Modal Submitted gives for each kind of answer.
// Uses discord.js when XC Circuit installed it on this computer, and small
// stand-ins with the same names otherwise (as on the build machines).
// Usage: node runtime/test/discord_forms.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const assert = require("assert");
const Module = require("module");
const { EventEmitter } = require("events");
const { ungroupBlock } = require("../circuit-runtime.js");

// ---- discord.js, or stand-ins ------------------------------------------------
const base = process.platform === "win32" ? process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming") : path.join(os.homedir(), ".local", "share");
const installed = path.join(base, "XC Circuit", "packages", "node_modules", "discord.js");
let discord;
let real = false;
if (fs.existsSync(installed)) {
  discord = require(installed);
  real = true;
} else {
  class Builder {
    constructor() {
      this.data = {};
    }
    toJSON() {
      return { ...this.data };
    }
  }
  const setter = (cls, ...names) => {
    for (const n of names) {
      const key = n[3].toLowerCase() + n.slice(4);
      cls.prototype[n] = function (v) {
        this.data[key] = v;
        return this;
      };
    }
    return cls;
  };
  class TextInputBuilder extends Builder {}
  setter(TextInputBuilder, "setCustomId", "setLabel", "setStyle", "setRequired", "setPlaceholder", "setValue", "setMinLength", "setMaxLength");
  class StringSelectMenuBuilder extends Builder {}
  setter(StringSelectMenuBuilder, "setCustomId", "setPlaceholder");
  class FileUploadBuilder extends Builder {}
  setter(FileUploadBuilder, "setCustomId", "setMinValues", "setMaxValues", "setRequired");
  class TextDisplayBuilder extends Builder {}
  setter(TextDisplayBuilder, "setContent");
  class LabelBuilder extends Builder {
    toJSON() {
      return { ...this.data, component: this.data.component?.toJSON() };
    }
  }
  setter(LabelBuilder, "setLabel", "setDescription");
  for (const n of ["setTextInputComponent", "setStringSelectMenuComponent", "setUserSelectMenuComponent", "setRoleSelectMenuComponent", "setChannelSelectMenuComponent", "setMentionableSelectMenuComponent", "setFileUploadComponent"]) {
    LabelBuilder.prototype[n] = function (c) {
      this.data.component = c;
      return this;
    };
  }
  class ActionRowBuilder extends Builder {
    addComponents(...c) {
      this.data.components = c;
      return this;
    }
    toJSON() {
      return { type: 1, components: this.data.components.map((c) => c.toJSON()) };
    }
  }
  class ModalBuilder extends Builder {
    constructor(data) {
      super();
      this.data = { components: [], ...(data || {}) };
    }
    addLabelComponents(...c) {
      this.data.components.push(...c);
      return this;
    }
    addTextDisplayComponents(...c) {
      this.data.components.push(...c);
      return this;
    }
    addActionRowComponents(...c) {
      this.data.components.push(...c);
      return this;
    }
    toJSON() {
      return { ...this.data, components: this.data.components.map((c) => (c.toJSON ? c.toJSON() : c)) };
    }
  }
  setter(ModalBuilder, "setCustomId", "setTitle");
  discord = { TextInputBuilder, TextInputStyle: { Short: 1, Paragraph: 2 }, StringSelectMenuBuilder, FileUploadBuilder, TextDisplayBuilder, LabelBuilder, ActionRowBuilder, ModalBuilder };
}
const load = Module._load;
Module._load = function (request, ...rest) {
  return request === "discord.js" ? discord : load.call(this, request, ...rest);
};

const dir = path.join(__dirname, "..", "..", "plugins", "xc.discord", "blocks");
const block = (rel) => ungroupBlock(require(path.join(dir, rel)));
const defaults = (b) => Object.fromEntries(Object.entries(b.fields || {}).filter(([, f]) => "default" in f).map(([k, f]) => [k, f.default]));
const run = (rel, fields) => {
  const b = block(rel);
  return b.run({ fields: { ...defaults(b), ...fields }, ctx: {} });
};
const json = (x) => JSON.parse(JSON.stringify(x.toJSON()));

(async () => {
  // A text box inside a Form Field keeps no label of its own.
  const { input: box } = run("interactions/text_input.js", { inputId: "why", label: "Old label" });
  const { field } = run("interactions/modal_field.js", { label: "Why do you want to join?", description: "A sentence is enough.", input: box });
  const f = json(field);
  assert.strictEqual(f.label, "Why do you want to join?");
  assert.strictEqual(f.description, "A sentence is enough.");
  assert.strictEqual(f.component.custom_id ?? f.component.customId, "why");
  assert.ok(!("label" in f.component), "the box's own label is gone");

  // A File Upload, and a menu on its own, get wrapped; Form Text stays text.
  const { input: upload } = run("interactions/file_upload.js", { inputId: "proof", min: 1, max: 3 });
  const menu = new discord.StringSelectMenuBuilder().setCustomId("team").setPlaceholder("Pick a team");
  if (real) menu.addOptions({ label: "Red", value: "red" }, { label: "Blue", value: "blue" });
  const { text } = run("interactions/modal_text.js", { text: "# Apply\nFill in everything." });
  assert.throws(() => run("interactions/modal_text.js", { text: "  " }), /needs some text/);
  assert.throws(() => run("interactions/modal_field.js", { input: { nope: true } }), /Connect a Text Input, Select Menu or File Upload/);

  let shown = null;
  const interaction = { showModal: async (m) => (shown = m) };
  const { input: old } = run("interactions/text_input.js", { inputId: "name", label: "Your name" });
  await run("interactions/show_modal.js", { interaction, customId: "apply", title: "Apply", inputs: [text, field, menu, upload, old] });
  const modal = json(shown);
  const kinds = modal.components.map((c) => (c.type === 10 || "content" in c ? "text" : c.type === 1 || Array.isArray(c.components) ? "row" : "label"));
  assert.deepStrictEqual(kinds, ["text", "label", "label", "label", "row"], "in order, each as Discord wants it");
  assert.strictEqual(modal.components[2].label, "Pick a team", "a menu on its own asks with its placeholder");
  assert.strictEqual(modal.components[3].label, "Attach a file");
  await assert.rejects(run("interactions/show_modal.js", { interaction, inputs: [{ odd: 1 }] }), /Inputs takes/);

  // A form from the Modal Builder: Discord's JSON, with ${…} filled in.
  const fill = (s, vars) => s.replace(/\$\{(\w+)\}/g, (_, k) => vars[k] ?? "");
  const showModal = block("interactions/show_modal.js");
  const form = { title: "Apply, ${name}", custom_id: "apply:${id}", components: [
    { type: 10, content: "Fill in everything." },
    { type: 18, label: "Why?", description: "A sentence", component: { type: 4, custom_id: "why", style: 2, required: true } },
    { type: 18, label: "Team", component: { type: 3, custom_id: "team", options: [{ label: "Red", value: "red" }] } },
    { type: 18, label: "Proof", component: { type: 19, custom_id: "proof", min_values: 1, max_values: 2 } },
  ] };
  await showModal.run({ fields: { ...defaults(showModal), interaction, form, name: "Ada", id: "42" }, ctx: { fill } });
  const built = json(shown);
  assert.strictEqual(built.title, "Apply, Ada");
  assert.strictEqual(built.custom_id, "apply:42");
  assert.deepStrictEqual(built.components.map((c) => c.type), [10, 18, 18, 18]);
  assert.deepStrictEqual(built.components[1].component, { type: 4, custom_id: "why", style: 2, required: true });
  assert.strictEqual(built.components[3].component.max_values, 2);
  // The block's default form works as it is; an empty one says what to do.
  await showModal.run({ fields: { ...defaults(showModal), interaction }, ctx: { fill } });
  assert.strictEqual(json(shown).components[0].component.custom_id, "answer");
  await assert.rejects(showModal.run({ fields: { ...defaults(showModal), interaction, form: { title: "T", components: [] } }, ctx: { fill } }), /no questions/);
  // Blocks wired into Inputs hide the form; otherwise the form hides Custom id and Title.
  assert.deepStrictEqual(showModal.define({ connected: {} }).hide, ["customId", "title"]);
  assert.deepStrictEqual(showModal.define({ connected: { inputs: "list<discord.component>" } }).hide, ["form"]);

  // Modal Submitted: text, menu values, picked users and files, by input id.
  const onModal = block("events/on_modal.js");
  const client = new EventEmitter();
  const out = [];
  onModal.listen({ fields: { ...defaults(onModal), customId: "apply", inputIds: "name, team, proof, who" }, discord: { client } }, (o) => out.push(o));
  const file = { name: "a.png", url: "https://cdn/a.png", size: 5 };
  const ada = { id: "1", username: "ada" };
  const fields = new Map([
    ["name", { value: "Ada" }],
    ["team", { values: ["red"] }],
    ["proof", { values: ["9"], attachments: new Map([["9", file]]) }],
    ["who", { values: ["1"], users: new Map([["1", ada]]) }],
  ]);
  client.emit("interactionCreate", { isModalSubmit: () => true, customId: "apply", user: ada, member: null, fields: { fields, getTextInputValue: (id) => fields.get(id).value } });
  await new Promise((r) => setImmediate(r));
  assert.strictEqual(out.length, 1);
  const o = out[0];
  assert.deepStrictEqual([o.name, o.team, o.proof, o.who], ["Ada", "red", "https://cdn/a.png", "1"]);
  assert.deepStrictEqual(o.answers.team, ["red"]);
  assert.deepStrictEqual(o.files, [file]);
  assert.deepStrictEqual(o.picked, { who: [ada] });

  console.log(`forms test passed${real ? " (with discord.js)" : " (with stand-ins)"}`);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
