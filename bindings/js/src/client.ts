import {
  DEFAULT_COLS,
  DEFAULT_ROWS,
  artifactPayload,
  assertTimeoutClasses,
  backendPayload,
  envPairs,
  profilePayload,
  recordingPayload,
  tracePayload,
  resolveSession,
  resolveTimeout,
  timeoutsPayload,
} from "./config.js";
import type { TimeoutClass } from "./config.js";
import { uniqueSession } from "./ephemeral.js";
import { ExpectationError, InternalError, UsageError } from "./errors.js";
import { NativeRuntime } from "./native.js";
import type {
  RuntimeLocatorNode,
  RuntimeLocatorExpression,
  RuntimeLocatorStyle,
} from "./native.js";
import type {
  BellEvent,
  Cell,
  ClientOptions,
  Cursor,
  Viewport,
  OpenResult,
  Shell,
  Size,
  SpawnOptions,
  State,
  TextMatch,
} from "./types.js";

export interface TitleOptions {
  regex?: boolean;
  not?: boolean;
  timeout?: number;
}

export interface ClipboardWaitOptions {
  timeout?: number;
}

function isRegExp(value: unknown): value is RegExp {
  return Object.prototype.toString.call(value) === "[object RegExp]";
}

type TextOccurrence =
  | "any"
  | "unique"
  | "first"
  | "last"
  | { nth: number };

export type LocatorDirection = "within" | "after" | "before";

export interface TextSelectorOptions {
  regex?: boolean;
  full?: boolean;
  whitespace?: "exact" | "normalize";
}

export interface RelativeTextSelectorOptions extends TextSelectorOptions {
  direction?: LocatorDirection;
}

export interface TextStyleExpectation {
  foreground?: string;
  background?: string;
  bold?: boolean;
  dim?: boolean;
  italic?: boolean;
  underlineStyle?: "none" | "single" | "double" | "curly" | "dotted" | "dashed";
  underlineColor?: string;
  inverse?: boolean;
  hidden?: boolean;
  strikethrough?: boolean;
  blink?: boolean;
}

export interface LocatorWaitOptions {
  state?: "visible" | "hidden";
  timeout?: number;
}

export type MouseButton = "left" | "middle" | "right";

export interface MouseButtonOptions {
  button?: MouseButton;
  alt?: boolean;
  ctrl?: boolean;
  shift?: boolean;
}

export interface MouseClickOptions extends MouseButtonOptions {
  onText?: string;
  clicks?: number;
}

export interface LocatorClickOptions extends MouseButtonOptions {
  clicks?: number;
  timeout?: number;
}

export interface LocatorHighlightOptions {
  timeout?: number;
}

export interface LocatorExpectOptions {
  not?: boolean;
  timeout?: number;
}

export interface StyleSelectorOptions {
  full?: boolean;
}

export interface RelativeStyleSelectorOptions extends StyleSelectorOptions {
  direction?: LocatorDirection;
}

export interface LinkSelectorOptions {
  full?: boolean;
}

export interface RelativeLinkSelectorOptions extends LinkSelectorOptions {
  direction?: LocatorDirection;
}

export interface LocatorFilterOptions {
  has?: Locator;
  hasNot?: Locator;
}

export interface Locator {
  getByText(text: string, opts?: RelativeTextSelectorOptions): Locator;
  getByStyle(style: TextStyleExpectation, opts?: RelativeStyleSelectorOptions): Locator;
  getByLink(uri: string, opts?: RelativeLinkSelectorOptions): Locator;
  and(other: Locator): Locator;
  or(other: Locator): Locator;
  filter(options: LocatorFilterOptions): Locator;
  any(): Locator;
  unique(): Locator;
  first(): Locator;
  last(): Locator;
  nth(index: number): Locator;
  locations(): Promise<TextMatch[]>;
  location(): Promise<TextMatch>;
  count(): Promise<number>;
  all(): Promise<Locator[]>;
  wait(opts?: LocatorWaitOptions): Promise<Locator>;
  click(opts?: LocatorClickOptions): Promise<void>;
  highlight(opts?: LocatorHighlightOptions): Promise<void>;
  expect(opts?: LocatorExpectOptions): Promise<void>;
}

export type RecordingFormat = "apng" | "gif" | "mp4" | "cast";

export interface RecordingOptions {
  format?: RecordingFormat;
  fps?: number;
  speed?: number;
  idleTimeLimit?: number;
  zoom?: number;
  background?: string;
  transparent?: boolean;
}

export interface ScreenshotOptions {
  full?: boolean;
  zoom?: number;
  background?: string;
  transparent?: boolean;
}

export interface RestartOptions {
  gracefulTimeout?: number;
}

function withOperation(error: unknown, operation: string): unknown {
  if (!(error instanceof ExpectationError)) {
    return error;
  }
  const previous = error.message;
  error.message = `${operation}: ${previous}`;
  if (error.stack) {
    error.stack = error.stack.replace(
      `${error.name}: ${previous}`,
      `${error.name}: ${error.message}`,
    );
  }
  return error;
}

function optional<T>(value: T | null | undefined): T | undefined {
  return value ?? undefined;
}

function isMouseButton(value: unknown): value is MouseButton {
  return value === "left" || value === "middle" || value === "right";
}

function mouseModifier(value: boolean | undefined, name: string, bit: number): number {
  if (value !== undefined && typeof value !== "boolean") {
    throw new TypeError(`${name} must be a boolean`);
  }
  return value ? bit : 0;
}

function mouseButtonCode(opts: MouseButtonOptions): number {
  const button = opts.button ?? "left";
  if (!isMouseButton(button)) {
    throw new TypeError(
      `unknown mouse button "${String(button)}"; expected one of left, middle, right`,
    );
  }
  const base = { left: 0, middle: 1, right: 2 }[button];
  return (
    base +
    mouseModifier(opts.shift, "shift", 4) +
    mouseModifier(opts.alt, "alt", 8) +
    mouseModifier(opts.ctrl, "ctrl", 16)
  );
}

class Keyboard {
  #runtime: NativeRuntime;

  constructor(runtime: NativeRuntime) {
    this.#runtime = runtime;
  }

  async press(...keys: string[]): Promise<void> {
    await this.#runtime.press(keys);
  }

  async down(...keys: string[]): Promise<void> {
    await this.#runtime.keyDown(keys);
