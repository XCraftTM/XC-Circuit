const KINDS = {
  entitlementCreate: "Something is bought",
  entitlementUpdate: "A purchase changes (renewed, cancelled)",
  entitlementDelete: "A purchase is removed (refunded)",
  subscriptionCreate: "A subscription starts",
  subscriptionUpdate: "A subscription changes",
  subscriptionDelete: "A subscription ends",
};
const STATUS = { 0: "active", 1: "ending", 2: "inactive" };

module.exports = {
  id: "discord.on_purchase",
  kind: "event",
  info: {
    name: "Purchase Made",
    description: "Runs when someone buys one of the bot's items (or a test item is given), when a purchase changes or is refunded, and when a subscription starts, changes or ends.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "events_bot",
    aliases: ["Entitlement created", "Subscription started", "Item bought", "Premium purchased"],
    tags: ["premium", "monetization", "sku", "entitlement", "subscription", "purchase", "paid"],
  },
  look: {
    icon: "key",
  },
  requires: {
    intents: ["Guilds"],
  },
  fields: {
    kind: { type: "select", pin: false, default: "entitlementCreate", label: "When", choices: KINDS, description: "Which event runs it." },
    sku: { type: "text", pin: false, default: "", label: "SKU id", description: "Only this item. Empty = any." },
  },
  outputs: {
    user: { type: "discord.user?", description: "Who bought it, when known." },
    server: { type: "discord.guild?", description: "The server it was bought for, if any." },
    skuId: { type: "text", label: "SKU id", description: "The item." },
    entitlementId: { type: "text", label: "Entitlement id", description: "The purchase (for Premium › Use up)." },
    status: { type: "text", description: "For subscriptions: active, ending or inactive." },
    test: { type: "boolean", description: "Whether it is a free test item." },
    data: { type: "object", description: "The whole entitlement or subscription." },
  },
  listen(ctx, emit) {
    const c = ctx.discord.client;
    const kind = ctx.fields.kind;
    const want = String(ctx.fields.sku || "").trim();
    const user = (id) => (id ? c.users.fetch(id).catch(() => null) : null);
    c.on(kind, async (...args) => {
      const now = args[kind.endsWith("Update") ? 1 : 0];
      if (kind.startsWith("entitlement")) {
        if (want && now.skuId !== want) return;
        emit({
          user: await user(now.userId),
          server: now.guildId ? (c.guilds.cache.get(now.guildId) ?? null) : null,
          skuId: now.skuId,
          entitlementId: now.id,
          status: "",
          test: typeof now.isTest === "function" ? now.isTest() : false,
          data: now,
        });
      } else {
        if (want && !(now.skuIds || []).includes(want)) return;
        emit({ user: await user(now.userId), server: null, skuId: (now.skuIds || [])[0] ?? "", entitlementId: (now.entitlementIds || [])[0] ?? "", status: STATUS[now.status] ?? String(now.status), test: false, data: now });
      }
    });
  },
};
