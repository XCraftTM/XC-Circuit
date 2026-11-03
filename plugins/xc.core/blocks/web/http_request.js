const METHODS = { GET: "GET", POST: "POST", PUT: "PUT", PATCH: "PATCH", DELETE: "DELETE" };

module.exports = {
  id: "core.http_request",
  kind: "action",
  info: {
    name: "Web Request",
    description: "Calls a web address (API) and gives back the answer. Fails on HTTP errors, so wrap it in a Try Zone.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "web",
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
    const res = await fetch(fields.url, init);
    const text = await res.text();
    let data = text;
    try {
      data = JSON.parse(text);
    } catch {}
    if (!res.ok) throw Object.assign(new Error(`${fields.method} ${fields.url} answered ${res.status}`), { code: res.status });
    return { status: res.status, data };
  },
};
