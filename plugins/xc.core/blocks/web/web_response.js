module.exports = {
  id: "core.web_response",
  kind: "action",
  info: {
    name: "Web Response",
    description: "Answers the request that started this flow (from Web Request Received).",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "web",
    tags: ["http", "reply", "api", "json"],
  },
  look: {
    icon: "send",
  },
  fields: {
    status: { type: "number", default: 200, min: 100, max: 599, step: 1, description: "The status code: 200 OK, 400 bad request, 404 not found …" },
    body: { type: "any", pin: true, default: "OK", description: "Text, or an object (sent as JSON)." },
    contentType: { type: "text", template: false, default: "", advanced: true, label: "Content type", description: "Such as text/html. Empty = text or JSON." },
  },
  run({ fields, ctx }) {
    const respond = ctx.event && ctx.event.__respond;
    if (typeof respond !== "function") throw new Error("Web Response only works in a flow started by Web Request Received");
    respond(Math.floor(fields.status || 200), fields.body, fields.contentType || undefined);
  },
};
