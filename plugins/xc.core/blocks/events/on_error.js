module.exports = {
  id: "core.on_error",
  kind: "event",
  info: {
    name: "On Error",
    description: "Runs when a block outside any Try Zone fails. Use it to log or report errors.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events",
  },
  look: {
    icon: "info",
  },
  outputs: {
    message: { type: "text", description: "What went wrong." },
    block: { type: "text", description: "Name of the block that failed." },
    details: { type: "object", description: "Error code, workspace, block id and stack trace." },
  },
  listen(ctx, emit) {
    ctx.runtime.onError((e) => emit(e));
  },
};
