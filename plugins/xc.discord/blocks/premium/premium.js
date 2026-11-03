const ACTIONS = {
  check: "Check if someone owns an item",
  skus: "List the items for sale (SKUs)",
  entitlements: "List what someone owns",
  consume: "Use up a consumable",
  subscriptions: "List someone's subscriptions",
  createTest: "Give a free test item",
  deleteTest: "Take a test item back",
};
const FOR = { user: "A user", server: "A server" };
const SKU_TYPES = { 2: "durable", 3: "consumable", 5: "subscription", 6: "subscription group" };
const STATUS = { 0: "active", 1: "ending", 2: "inactive" };

const entitlementInfo = (e) => ({
  id: e.id,
  skuId: e.skuId,
  userId: e.userId ?? "",
  serverId: e.guildId ?? "",
  active: typeof e.isActive === "function" ? e.isActive() : !e.deleted,
  consumed: Boolean(e.consumed),
  test: typeof e.isTest === "function" ? e.isTest() : false,
  endsAt: e.endsTimestamp ? new Date(e.endsTimestamp).toISOString() : "",
});

module.exports = {
  id: "discord.premium",
  kind: "action",
  info: {
    name: "Premium",
    description: "Sell things with Discord's monetization: checks whether a user or server owns an item (SKU), lists items, purchases and subscriptions, uses up consumables, and gives test items while developing. Set up items in the developer portal › Monetization.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "premium",
    aliases: ["Has premium", "Check purchase", "Entitlements", "SKUs", "Subscriptions"],
    tags: ["premium", "monetization", "sku", "entitlement", "subscription", "purchase", "shop", "paid"],
  },
  look: {
    icon: "key",
  },
  fields: {
    action: { type: "select", pin: false, default: "check", choices: ACTIONS, description: "What to do." },
    for: { type: "select", pin: false, default: "user", label: "Owner", choices: FOR, show: { action: ["check", "entitlements", "createTest"] }, description: "Items bought by a user, or for a server." },
    user: { type: "discord.user?", pin: "only", show: { action: ["check", "entitlements", "createTest", "subscriptions"] }, description: "The user." },
    server: { type: "discord.guild?", pin: "only", show: { action: ["check", "entitlements", "createTest"], for: ["server"] }, description: "The server." },
    sku: { type: "text", default: "", label: "SKU id", show: { action: ["check", "entitlements", "createTest", "subscriptions"] }, description: "The item. For lists: empty = all." },
    entitlement: { type: "text", default: "", label: "Entitlement id", show: { action: ["consume", "deleteTest"] }, description: "The purchase (from a list or Purchase Made)." },
  },
  outputs: {
    owns: { type: "boolean", description: "For check: whether the item is owned and active." },
    entitlement: { type: "object", description: "For check and test items: the purchase." },
    list: { type: "list<object>", description: "For lists: items (id, name, type), purchases (id, skuId, userId, serverId, active, consumed, test, endsAt) or subscriptions (id, status, skuIds, renewsAt, canceledAt)." },
  },
  async run({ fields: f, ctx }) {
    const app = ctx.discord.client.application;
    const none = { owns: false, entitlement: null, list: [] };
    const sku = String(f.sku || "").trim();
    const owner = () => {
      if (f.for === "server") {
        if (!f.server) throw new Error("Connect a server");
        return { guild: f.server };
      }
      if (!f.user) throw new Error("Connect a user");
      return { user: f.user };
    };
    switch (f.action) {
      case "skus": {
        const skus = await app.fetchSKUs();
        return { ...none, list: [...skus.values()].map((s) => ({ id: s.id, name: s.name, type: SKU_TYPES[s.type] ?? String(s.type), slug: s.slug })) };
      }
      case "check":
      case "entitlements": {
        const all = await app.entitlements.fetch({ ...owner(), skus: sku ? [sku] : undefined, excludeEnded: f.action === "check", excludeDeleted: true, cache: false });
        const list = [...all.values()];
        const active = list.find((e) => (sku ? e.skuId === sku : true) && (typeof e.isActive === "function" ? e.isActive() : true));
        return { owns: Boolean(active), entitlement: active ?? null, list: list.map(entitlementInfo) };
      }
      case "consume": {
        await app.entitlements.consume(String(f.entitlement).trim());
        return none;
      }
      case "subscriptions": {
        if (!f.user || !sku) throw new Error("Connect a user and give the subscription's SKU id");
        const subs = await app.subscriptions.fetch({ user: f.user, sku });
        const list = [...subs.values()].map((s) => ({
          id: s.id,
          status: STATUS[s.status] ?? String(s.status),
          skuIds: s.skuIds,
          renewsAt: s.currentPeriodEndTimestamp ? new Date(s.currentPeriodEndTimestamp).toISOString() : "",
          canceledAt: s.canceledTimestamp ? new Date(s.canceledTimestamp).toISOString() : "",
        }));
        return { ...none, owns: list.some((s) => s.status !== "inactive"), list };
      }
      case "createTest": {
        if (!sku) throw new Error("Give the SKU id");
        const e = await app.entitlements.createTest({ sku, ...owner() });
        return { ...none, owns: true, entitlement: e };
      }
      case "deleteTest": {
        await app.entitlements.deleteTest(String(f.entitlement).trim());
        return none;
      }
    }
    throw new Error(`Unknown action ${f.action}`);
  },
};
