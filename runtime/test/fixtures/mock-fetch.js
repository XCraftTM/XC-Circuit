// Loaded with node -r before a test bot: fetch() answers from the JSON file
// in XC_MOCK_FETCH ({ "url prefix": { status?, body } }, read on every call,
// so a test can change the answers while the bot runs) and logs each URL it
// was asked for to XC_MOCK_FETCH + ".log".
"use strict";

const fs = require("fs");

const file = process.env.XC_MOCK_FETCH;
globalThis.fetch = async (url, init = {}) => {
  url = String(url);
  fs.appendFileSync(`${file}.log`, `${init.method || "GET"} ${url}\n`);
  const answers = JSON.parse(fs.readFileSync(file, "utf8"));
  const key = Object.keys(answers)
    .filter((k) => url.startsWith(k))
    .sort((a, b) => b.length - a.length)[0];
  const a = key ? answers[key] : { status: 404, body: "not found" };
  const text = typeof a.body === "string" ? a.body : JSON.stringify(a.body);
  const status = a.status || 200;
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: (name) => (String(name).toLowerCase() === "content-length" ? String(Buffer.byteLength(text)) : null) },
    text: async () => text,
    json: async () => JSON.parse(text),
    arrayBuffer: async () => Buffer.from(text),
  };
};
