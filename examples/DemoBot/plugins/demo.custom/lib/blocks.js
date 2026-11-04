// Helpers for editor.js, loaded with require("./lib/blocks.js").

/** A block's text values as [field, text] pairs. */
function textValues(block) {
  return Object.entries(block.values || {}).filter(([, v]) => typeof v === "string" && v.trim() !== "");
}

module.exports = { textValues };
