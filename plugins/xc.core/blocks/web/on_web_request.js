// Port -> { server, routes: [{ method, path, handle }] }, shared by every block in the bot.
const SERVERS = new Map();

function serve(port, log) {
  if (SERVERS.has(port)) return SERVERS.get(port);
  const http = require("http");
  const entry = { routes: [] };
  entry.server = http.createServer((req, res) => {
    const url = new URL(req.url, "http://localhost");
    const route = entry.routes.find((r) => (r.method === "ANY" || r.method === req.method) && r.path === url.pathname);
    if (!route) {
      res.writeHead(404).end("Not found");
      return;
    }
    let body = "";
    req.on("data", (d) => {
      body += d;
      if (body.length > 1e6) req.destroy();
    });
    req.on("end", () => route.handle(req, res, url, body));
  });
  entry.server.listen(port, () => log.info(`Listening for web requests on port ${port}`));
  entry.server.on("error", (e) => log.error(`Web requests on port ${port}: ${e.message}`));
  SERVERS.set(port, entry);
  return entry;
}

module.exports = {
  id: "core.on_web_request",
  kind: "event",
  info: {
    name: "Web Request Received",
    description: "Runs when a web address of the bot is called, such as by a dashboard, a webhook from another service or a form. Answer with Web Response.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "web",
    aliases: ["Webhook received", "HTTP server"],
    tags: ["http", "server", "api", "webhook", "endpoint", "dashboard"],
  },
  look: {
    icon: "send",
  },
  fields: {
    port: { type: "number", pin: false, default: 3000, min: 1, max: 65535, step: 1, description: "The port to listen on (http://your-server:3000)." },
    path: { type: "text", pin: false, template: false, default: "/hook", description: "The path, such as /hook." },
    method: { type: "select", pin: false, default: "POST", choices: { POST: "POST", GET: "GET", PUT: "PUT", DELETE: "DELETE", ANY: "Any" }, description: "Which kind of request." },
    secret: { type: "text", pin: false, template: false, default: "", advanced: true, description: "When set, requests must send it as ?key=… or an x-api-key header." },
  },
  outputs: {
    body: { type: "any", description: "What was sent: an object for JSON, otherwise text." },
    query: { type: "object", description: "The ?name=value parts of the address." },
    headers: { type: "object", description: "The request's headers." },
    method: { type: "text", description: "GET, POST …" },
  },
  listen(ctx, emit) {
    const f = ctx.fields;
    const method = String(f.method || "POST").toUpperCase();
    serve(Number(f.port) || 3000, ctx.log).routes.push({
      method,
      path: String(f.path || "/").startsWith("/") ? f.path : "/" + f.path,
      handle: async (req, res, url, raw) => {
        if (f.secret && url.searchParams.get("key") !== f.secret && req.headers["x-api-key"] !== f.secret) {
          res.writeHead(401).end("Unauthorized");
          return;
        }
        let body = raw;
        try {
          body = raw ? JSON.parse(raw) : {};
        } catch {}
        let answered = false;
        const respond = (status, data, type) => {
          if (answered) return;
          answered = true;
          const text = typeof data === "string" ? data : JSON.stringify(data ?? {});
          res.writeHead(status, { "content-type": type || (typeof data === "string" ? "text/plain; charset=utf-8" : "application/json") }).end(text);
        };
        await emit({ body, query: Object.fromEntries(url.searchParams), headers: req.headers, method: req.method, __respond: respond });
        respond(204, "");
      },
    });
  },
};
