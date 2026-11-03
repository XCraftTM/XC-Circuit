// Types for XC Circuit block files. See the Plugin & Block API spec.

/** A type string: "text", "number", "boolean", "date", "any", "object", "list<T>", "T?", "A | B", or a plugin type such as "discord.member". */
export type TypeString = string;

export interface FieldDef {
  type: TypeString | "select" | "textarea" | "color" | "code" | "embed" | "slash-command-ref";
  label?: string;
  description?: string;
  default?: unknown;
  required?: boolean;
  /** "auto" (default): inline with a pin toggle · true: starts as a pin · false: inline only · "only": must be connected. */
  pin?: boolean | "auto" | "only";
  multi?: boolean;
  advanced?: boolean;
  /** Shown only when other fields have these values, e.g. { mode: ["edit", "reply"] }. */
  show?: Record<string, unknown>;
  choices?: Record<string, string | Record<string, string>>;
  template?: boolean;
  min?: number;
  max?: number;
  step?: number;
  placeholder?: string;
}

export interface OutputDef {
  type: TypeString;
  label?: string;
  description?: string;
}

export interface DefineArgs {
  values: Record<string, any>;
  /** Type strings of what is connected to each input pin. */
  connected: Record<string, TypeString>;
  project: { data?: Record<string, any> };
}

export interface DefineResult {
  add?: Record<string, FieldDef>;
  hide?: string[];
  set?: Record<string, Partial<FieldDef>>;
  outputs?: Record<string, OutputDef>;
  flows?: string[];
}

export interface Store {
  get<T = any>(key: string, fallback?: T): Promise<T>;
  set(key: string, value: unknown): Promise<void>;
  has(key: string): Promise<boolean>;
  delete(key: string): Promise<void>;
  inc(key: string, by?: number): Promise<number>;
  push(key: string, value: unknown): Promise<unknown[]>;
}

export interface Vars {
  get<T = any>(name: string, fallback?: T): T;
  set(name: string, value: unknown): void;
  has(name: string): boolean;
  delete(name: string): boolean;
}

export interface Ctx {
  next(flow: string, outputs?: Record<string, unknown>): Promise<void>;
  set(output: string, value: unknown): void;
  log: Record<"info" | "success" | "warn" | "error" | "debug", (...parts: unknown[]) => void>;
  vars: { run: Vars; workspace: Vars; global: Vars };
  store: {
    global(): Store;
    server(id: unknown): Store;
    user(id: unknown): Store;
    member(serverId: unknown, userId: unknown): Store;
    channel(id: unknown): Store;
    ns(name: string): Store;
  };
  signal: { emit(name: string, data?: unknown): void };
  /** Fills {placeholders} from the event, run variables, earlier outputs and `vars`. */
  fill(text: string, vars?: Record<string, unknown>): string;
  event: Record<string, unknown> | undefined;
  fields: Record<string, any>;
  project: { name: string; mode: string; settings: Record<string, unknown>; data: Record<string, any> };
  util: { sleep(ms: number): Promise<void>; isURL(s: string): boolean; id(n?: number): string; shuffle<T>(list: T[]): T[]; typeOf(v: unknown): string };
  bot: { readonly uptime: number; stop(): void; restart(): void };
  runtime: { onError(h: (e: unknown) => void): void; onSignal(name: string, h: (data: unknown) => void): void; onStart(h: () => void): void; setting(name: string): unknown };
  abortSignal?: AbortSignal;
  require(name: string): any;
  /** Added by the Discord Bot mode. */
  discord?: { client: any };
}

/** What the block is. */
export interface BlockInfo {
  name: string;
  description: string;
  author: string;
  version: string;
  docs?: string;
  deprecated?: string | boolean;
}

/** How it is found in the block picker. */
export interface BlockSearch {
  /** A category id of the build mode, such as "send" or "moderation". */
  category?: string;
  tags?: string[];
}

/** How it looks on the canvas. */
export interface BlockLook {
  icon?: string;
  width?: number;
  preview?: string;
}

/** What it needs to run. */
export interface BlockRequires {
  /** Discord gateway intents, such as "GuildMessages". */
  intents?: string[];
  /** npm packages: name → version. */
  dependencies?: Record<string, string>;
}

/**
 * A block. What describes it goes in the groups `info`, `search`, `look`
 * and `requires`; each of their keys may also stand on its own at the top.
 */
export interface BlockDef<F = Record<string, any>> extends Partial<BlockInfo>, BlockSearch, BlockLook, BlockRequires {
  id: string;
  kind: "event" | "action" | "value";
  info?: BlockInfo;
  search?: BlockSearch;
  look?: BlockLook;
  requires?: BlockRequires;
  fields?: Record<string, FieldDef>;
  outputs?: Record<string, OutputDef>;
  flows?: string[];
  pure?: boolean;
  define?(args: DefineArgs): DefineResult;
  migrate?(values: Record<string, any>, fromVersion: string): Record<string, any>;
  setup?(ctx: Ctx): void | Promise<void>;
  listen?(ctx: Ctx, emit: (outputs: Record<string, unknown>) => void): void;
  run?(args: { fields: F; ctx: Ctx; node: { id: string; workspace: string } }): unknown;
}

export function defineBlock<F = Record<string, any>>(block: BlockDef<F>): BlockDef<F>;
