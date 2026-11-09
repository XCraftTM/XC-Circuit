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
  /** Used less often: on the block below More options (folded, shown only while in use), in the Inspector under More options. */
  advanced?: boolean;
  /** For a field define() adds: the key of the field it goes after, or "" for the very top. Without it, it goes last. */
  after?: string;
  /** Shown only when other fields have these values, e.g. { mode: ["edit", "reply"] }. */
  show?: Record<string, unknown>;
  choices?: Record<string, string | Record<string, string>>;
  template?: boolean;
  min?: number;
  max?: number;
  step?: number;
  placeholder?: string;
  /** With `choices`: the list ends in Custom…, which shows a text box for any other value. */
  custom?: boolean;
  /** An event's custom id field: offers the project's ids of this kind. */
  suggest?: "button" | "select" | "form";
  /** This field gives a button, menu or form its custom id. */
  customIdOf?: "button" | "select" | "form";
  /** An id or name that can be picked from the bot's servers. Channel id fields get it from their type. */
  pick?: "channel" | "voice" | "category" | "role";
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

/** Stored data of one scope (see ctx.store). Every method returns a promise. */
export interface Store {
  get<T = any>(key: string, fallback?: T): Promise<T>;
  set(key: string, value: unknown): Promise<void>;
  has(key: string): Promise<boolean>;
  delete(key: string): Promise<void>;
  /** Adds to a number (from 0) and gives the new number. */
  inc(key: string, by?: number): Promise<number>;
  /** Appends to a list (from empty) and gives the list. */
  push(key: string, value: unknown): Promise<unknown[]>;
  /** Every key in this scope. */
  keys(): Promise<string[]>;
}

/** Variables of one scope (see ctx.vars). Gone when the bot stops. */
export interface Vars {
  get<T = any>(name: string, fallback?: T): T;
  set(name: string, value: unknown): void;
  has(name: string): boolean;
  delete(name: string): boolean;
}

/** What blocks get as `ctx` while the bot runs. */
export interface Ctx {
  /** Continues on one of the block's `flows`, after setting `outputs`. Resolves when the rest of the flow is done. */
  next(flow: string, outputs?: Record<string, unknown>): Promise<void>;
  /** Whether one of the block's `flows` has a wire: take an optional path (Missing) only when it's connected. */
  wired?(flow: string): boolean;
  /** Sets one output without continuing. */
  set(output: string, value: unknown): void;
  /** Reads the block's inputs again (for loops whose condition changes). */
  resolve(): Promise<Record<string, any>>;
  /** Sets what this run gives back to the Call block (or emit()) that started it. */
  returnValue(value: unknown): void;
  log: Record<"info" | "success" | "warn" | "error" | "debug", (...parts: unknown[]) => void>;
  vars: { run: Vars; workspace: Vars; global: Vars };
  store: {
    global(): Store;
    server(id: unknown): Store;
    user(id: unknown): Store;
    member(serverId: unknown, userId: unknown): Store;
    channel(id: unknown): Store;
    /** Your own namespace, to keep a plugin's data apart. */
    ns(name: string): Store;
    /** Every stored entity of a scope, for leaderboards: [{ id, values }]. */
    each(scope: "global" | "server" | "user" | "member" | "channel" | "ns", serverId?: unknown): { id: string; values: Record<string, unknown> }[];
  };
  /** Starts every On Signal event with that name, without waiting. */
  signal: { emit(name: string, data?: unknown): void };
  /** Fills ${…} in a text like text fields are filled: from the event, run variables, earlier outputs and `vars`. */
  fill(text: string, vars?: Record<string, unknown>): string;
  /** The outputs of the event that started this run. */
  event: Record<string, unknown> | undefined;
  /** The block's own field values as filled in (without connected values). */
  fields: Record<string, any>;
  project: { name: string; mode: string; settings: Record<string, unknown>; data: Record<string, any> };
  util: {
    sleep(ms: number): Promise<void>;
    isURL(s: string): boolean;
    id(n?: number): string;
    shuffle<T>(list: T[]): T[];
    typeOf(v: unknown): string;
    /** Compare's checks: "eq", "ne", "gt", "lt", "gte", "lte", "between", "contains", "starts", "ends", "matches"… as in the Compare block. */
    compare(a: unknown, check: string, b: unknown, options?: { c?: unknown; ignoreCase?: boolean }): boolean;
  };
  bot: { readonly uptime: number; stop(): void; restart(): void };
  runtime: {
    onError(h: (e: unknown) => void): void;
    onSignal(name: string, h: (data: unknown) => void): void;
    /** Answers Call blocks with that name: h(data) starts a run and resolves to what it returns. */
    onCall(name: string, h: (data: unknown) => Promise<unknown>): void;
    /** Runs the When Called flows with that name and resolves to the first one's return value. */
    call(name: string, data?: unknown): Promise<unknown>;
    onStart(h: () => void): void;
    setting(name: string): unknown;
  };
  /** Inside a Try Zone with a timeout: fires when the zone gives up. */
  abortSignal?: AbortSignal;
  /** Loads an npm package listed in the block's requires.dependencies (or Node's own modules). */
  require(name: string): any;
  /** Added by the Discord Bot mode. */
  discord?: { client: any };
  /** Added by the Fluxer Bot mode. */
  fluxer?: { client: any; prefix(): string; [helper: string]: any };
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
  /** Other English names it is found by, such as "Reply". They rank like the name and aren't shown. */
  aliases?: string[];
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
  /**
   * What the block does. Value blocks and actions without `flows` return their outputs;
   * actions with `flows` call ctx.next(). Write it `async` only when it awaits something:
   * the block picker tags blocks that wait as Async.
   */
  run?(args: { fields: F; ctx: Ctx; node: { id: string; workspace: string } }): unknown;
}

export function defineBlock<F = Record<string, any>>(block: BlockDef<F>): BlockDef<F>;
