const STYLES = { plain: "1,234.5", decimals: "Fixed decimals (1,234.50)", percent: "Percent (12%)", currency: "Currency ($1,234.50)", compact: "Short (1.2K)", ordinal: "Place (1st, 2nd)" };

function ordinal(n) {
  const s = ["th", "st", "nd", "rd"];
  const v = Math.abs(Math.trunc(n)) % 100;
  return `${Math.trunc(n)}${s[(v - 20) % 10] || s[v] || s[0]}`;
}

module.exports = {
  id: "core.format_number",
  kind: "value",
  info: {
    name: "Format Number",
    description: "Writes a number nicely: with separators, as percent, money, 1.2K or 1st.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "numbers",
    aliases: ["Number format", "Ordinal", "Zahl formatieren"],
    tags: ["thousands", "currency", "money", "percent", "ordinal", "round"],
  },
  look: {
    icon: "hash",
  },
  fields: {
    number: { type: "number", pin: true, default: 0, description: "The number." },
    style: { type: "select", pin: false, default: "plain", choices: STYLES, description: "How to write it." },
    decimals: { type: "number", default: 2, min: 0, max: 10, step: 1, show: { style: ["decimals", "currency", "percent"] }, description: "Digits after the point." },
    currency: { type: "text", default: "USD", show: { style: ["currency"] }, description: "A currency code such as USD, EUR or GBP." },
    locale: { type: "text", default: "en-US", advanced: true, description: "Language and region for separators, such as de-DE." },
  },
  outputs: {
    text: { type: "text", description: "The written number." },
  },
  run({ fields: f }) {
    const n = Number(f.number) || 0;
    const d = Math.max(0, Math.min(10, Math.floor(f.decimals ?? 2)));
    const locale = f.locale || "en-US";
    const fmt = (o) => new Intl.NumberFormat(locale, o).format(f.style === "percent" ? n / 100 : n);
    const text = {
      plain: () => fmt({ maximumFractionDigits: 10 }),
      decimals: () => fmt({ minimumFractionDigits: d, maximumFractionDigits: d }),
      percent: () => fmt({ style: "percent", maximumFractionDigits: d }),
      currency: () => fmt({ style: "currency", currency: String(f.currency || "USD").toUpperCase(), minimumFractionDigits: d, maximumFractionDigits: d }),
      compact: () => fmt({ notation: "compact", maximumFractionDigits: 1 }),
      ordinal: () => ordinal(n),
    }[f.style] || (() => String(n));
    return { text: text() };
  },
};
