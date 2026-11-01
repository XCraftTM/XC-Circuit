module.exports = {
  id: "core.throw",
  name: "Throw Error",
  description: "Stops this flow with an error. Inside a Try Zone, the zone's Error output runs.",
  author: "XC Team",
  version: "1.0.0",
  category: "debug",
  icon: "info",
  kind: "action",
  fields: {
    message: { type: "text", template: true, default: "Something went wrong", description: "The error message." },
  },
  run({ fields }) {
    throw new Error(fields.message);
  },
};
