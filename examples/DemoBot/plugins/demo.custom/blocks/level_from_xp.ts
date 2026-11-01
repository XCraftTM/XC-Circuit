// A project-local block written in TypeScript. XC Circuit strips the types when
// loading it; the types come from the @xc-circuit/sdk package (see /sdk).
import { defineBlock } from "@xc-circuit/sdk";

interface Fields {
  xp: number;
  perLevel: number;
}

export default defineBlock({
  id: "demo.level_from_xp",
  name: "Level From XP",
  description: "Turns an XP total into a level and the XP still needed for the next one.",
  author: "You",
  version: "1.0.0",
  category: "math",
  icon: "hash",
  kind: "value",
  fields: {
    xp: { type: "number", pin: true, description: "The XP total." },
    perLevel: { type: "number", default: 100, label: "XP per level" },
  },
  outputs: {
    level: { type: "number", description: "The level, starting at 1." },
    toNext: { type: "number", label: "XP to next", description: "XP still needed for the next level." },
  },
  run({ fields }: { fields: Fields }): { level: number; toNext: number } {
    const per: number = Math.max(1, fields.perLevel || 100);
    const xp: number = Math.max(0, Number(fields.xp) || 0);
    return { level: Math.floor(xp / per) + 1, toNext: per - (xp % per) };
  },
});
