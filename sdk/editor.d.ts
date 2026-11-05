// Types for XC Circuit editor plugins: the global `xc` an editor script
// (plugin.toml › [editor] script) uses. Reference it from your editor.js or
// editor.ts:  /// <reference types="@xc-circuit/sdk/editor" />

/** A problem for the Problems panel. With workspace and block, clicking it selects the block. */
export type Problem = string | { message: string; workspace?: string; block?: string };

/** A colour by name or as "#RRGGBB". */
export type Color = "accent" | "success" | "warning" | "error" | "muted" | `#${string}`;

/** A block as xc.blocks() gives it. */
export interface BlockInfo {
  workspace: string;
  id: string;
  /** The block id without its version: "core.log". */
  type: string;
  /** With its major version: "core.log@1". */
  block: string;
  at: [number, number];
  values: Record<string, any>;
  pins?: string[];
  zone?: string;
}

/** A workspace as saved (see the project format in the docs). */
export interface Workspace {
  name: string;
  folder?: string;
  nodes: Record<string, { block: string; at: [number, number]; values?: Record<string, any>; pins?: string[]; zone?: string }>;
  links: string[];
  zones?: Record<string, { title: string; rect: [number, number, number, number]; retry: number; timeoutMs?: number }>;
  notes?: Record<string, { title: string; text?: string; rect: [number, number, number, number]; color?: string; loose?: boolean }>;
}

export type BotEvent =
  | { type: "started" }
  | { type: "stopped" }
  | { type: "log"; level: "info" | "success" | "warn" | "error"; text: string }
  | { type: "block"; workspace: string; block: string; phase: "enter" | "exit" | "error"; ms?: number; error?: string };

/** What render(ui) uses to describe a panel or overlay. Every method returns the ui, to chain calls. */
export interface Ui {
  heading(text: unknown): Ui;
  text(text: unknown, options?: { muted?: boolean; bold?: boolean; mono?: boolean; size?: number; color?: Color }): Ui;
  muted(text: unknown): Ui;
  stat(label: unknown, value: unknown): Ui;
  separator(): Ui;
  space(points?: number): Ui;
  button(label: unknown, onClick?: () => void, options?: { primary?: boolean; tooltip?: string }): Ui;
  link(label: unknown, onClick?: () => void, options?: { tooltip?: string }): Ui;
  checkbox(label: unknown, checked: boolean, onChange?: (checked: boolean) => void): Ui;
  input(value: unknown, onChange?: (text: string) => void, options?: { placeholder?: string; multiline?: boolean; key?: string }): Ui;
  progress(fraction: number, label?: unknown): Ui;
  /** A picture from the plugin's folder (PNG, JPEG or SVG). */
  image(file: string, options?: { width?: number; height?: number; tint?: Color }): Ui;
  row(fill: (ui: Ui) => void): Ui;
  section(title: unknown, fill: (ui: Ui) => void, options?: { open?: boolean }): Ui;
}

export interface Xc {
  /** Adds a command to the Plugins menu and the command palette. */
  command(def: { id: string; title?: string; description?: string; run(): void }): void;
  /** Adds a dockable panel. render(ui) runs whenever it's drawn. */
  panel(def: { id: string; title?: string; render(ui: Ui): void }): void;
  /** Adds a check; what it returns shows in the Problems panel. */
  check(fn: () => Problem | Problem[] | null | undefined | void): void;
  /** Floats something over the whole editor. x, y: where it starts (negative: from the right or bottom edge). */
  overlay(def: { id: string; width: number; height: number; x?: number; y?: number; frame?: boolean; draggable?: boolean; render(ui: Ui): void; onClick?(): void }): void;
  moveOverlay(id: string, x: number, y: number): void;
  overlayPosition(id: string): { x: number; y: number } | null;
  /** The editor window's size. */
  screen(): { width: number; height: number };
  /** Runs fn every `ms` milliseconds (at least 50). */
  every(ms: number, fn: () => void): number;
  /** Runs fn once after `ms` milliseconds. */
  after(ms: number, fn: () => void): number;
  cancel(id: number): void;
  /** Plays a sound file of the plugin (WAV, MP3 or Ogg). */
  playSound(file: string, options?: { volume?: number }): void;
  on(event: "save", fn: () => void): void;
  on(event: "botEvent", fn: (event: BotEvent) => void): void;
  on(event: "selection", fn: (event: { workspace: string | null; blocks: string[] }) => void): void;

  project(): { name: string; root: string | null } | null;
  workspaces(): { key: string; name: string }[];
  workspace(key: string): Workspace | null;
  /** Replaces a workspace; it can be undone. */
  setWorkspace(key: string, workspace: Workspace): void;
  blocks(): BlockInfo[];
  activeWorkspace(): string | null;
  selection(): string[];
  select(workspace: string, blocks: string | string[]): void;
  reveal(workspace: string, block: string): void;
  /** Places a block and returns its id; it can be undone. */
  addBlock(workspace: string, type: string, options?: { x?: number; y?: number; values?: Record<string, unknown> }): string;
  blockName(type: string): string | null;
  /** The project's data, saved in circuit.json. */
  data: { get(key: string): any; set(key: string, value: unknown): void };
  /** This plugin's own store, on this computer. */
  storage: { get(key: string): any; set(key: string, value: unknown): void };
  log(...parts: unknown[]): void;
  success(...parts: unknown[]): void;
  warn(...parts: unknown[]): void;
  error(...parts: unknown[]): void;
  /** Runs a command after this handler: a plugin's, or "bot.run", "bot.stop", "file.save". */
  run(id: string): void;
  botRunning(): boolean;
  plugin: { name: string };
}

declare global {
  const xc: Xc;
}
