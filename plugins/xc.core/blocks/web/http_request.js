const METHODS = { GET: "GET", POST: "POST", PUT: "PUT", PATCH: "PATCH", DELETE: "DELETE" };

module.exports = {
  id: "core.http_request",
  kind: "action",
  info: {
    name: "Web Request",
    description: "Calls a web address (API) and gives back the answer. Fails on HTTP errors, so wrap it in a Try Zone.",
    author: "XC Team",
    version: "1.1.0",
  },
  search: {
    category: "web",
    aliases: ["Fetch", "API call", "HTTP"],
    tags: ["http", "api", "fetch", "url", "json"],
  },
  look: {
    icon: "send",
  },
  fields: {
    url: { type: "text", template: true, required: true, default: "https://", label: "URL", description: "The address. ${…} works." },
    method: { type: "select", pin: false, default: "GET", choices: METHODS, description: "GET reads, POST sends new data, PUT and PATCH change it, DELETE removes it." },
    body: { type: "any", pin: true, description: "Sent as JSON for POST, PUT and PATCH.", show: { method: ["POST", "PUT", "PATCH"] } },
    headers: { type: "textarea", pin: false, default: "", advanced: true, description: "One per line: Name: value" },
    timeoutSeconds: { type: "number", default: 15, advanced: true, label: "Timeout (s)", description: "Give up and fail when there is no answer after this many seconds." },
    maxMb: { type: "number", default: 10, min: 1, max: 500, advanced: true, label: "At most (MB)", description: "A bigger answer isn't read: the block stops with an error." },
    privateOk: { type: "boolean", pin: false, default: false, advanced: true, label: "Private addresses too", description: "An address from ${…} or a wire may only go to the public web, not this computer or its network (people could otherwise make the bot reach those). On: anywhere, for addresses only you choose." },
  },
  outputs: {
    status: { type: "number", description: "The HTTP status, such as 200." },
    data: { type: "any", description: "The answer: an object when it is JSON, otherwise text." },
  },
  async run({ fields, ctx }) {
    const headers = {};
    for (const line of String(fields.headers || "").split(/\r?\n/)) {
      const i = line.indexOf(":");
      if (i > 0) headers[line.slice(0, i).trim()] = line.slice(i + 1).trim();
    }
    const init = { method: fields.method, headers, signal: AbortSignal.timeout((fields.timeoutSeconds || 15) * 1000) };
    if (["POST", "PUT", "PATCH"].includes(fields.method) && fields.body !== undefined) {
      init.body = typeof fields.body === "string" ? fields.body : JSON.stringify(fields.body);
      headers["Content-Type"] ??= "application/json";
    }
    // An address the author typed goes anywhere; one from ${…} or a wire only to the public web.
    const publicOnly = !ctx.authored("url") && !fields.privateOk;
    const { res, body } = await ctx.util.fetch(fields.url, init, { publicOnly, maxBytes: (Number(fields.maxMb) || 10) * 1024 * 1024 });
    const text = body.toString("utf8");
    let data = text;
    try {
      data = JSON.parse(text);
    } catch {}
    if (!res.ok) throw Object.assign(new Error(`${fields.method} ${fields.url} answered ${res.status}`), { code: res.status });
    return { status: res.status, data };
  },
};
