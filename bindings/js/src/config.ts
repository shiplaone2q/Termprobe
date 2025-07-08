import path from "node:path";

import type {
  ArtifactOptions,
  AutomaticRecording,
  TraceOptions,
  Backend,
  Profile,
  Timeouts,
} from "./types.js";

export const DEFAULT_COLS = 80;
export const DEFAULT_ROWS = 30;

export const IS_WINDOWS = process.platform === "win32";
export const IS_MACOS = process.platform === "darwin";

export function resolveSession(session?: string): string {
  return session || process.env.TERMPROBE__SESSION || "default";
}

export type TimeoutClass = "text" | "idle" | "command" | "exit" | "ready";

const TIMEOUT_CLASSES: readonly TimeoutClass[] = [
  "text",
  "idle",
  "command",
  "exit",
  "ready",
];
const BACKENDS: readonly Backend[] = ["alacritty", "ghostty", "rio", "xtermjs"];
const PROFILE_FIELDS = new Set(["scrollback", "colors"]);
const TRACE_MODES = new Set(["off", "on-failure", "on"]);
const FAILURE_ARTIFACT_MODES = new Set([
  "all",
  "html",
  "text",
  "none",
]);
const COLOR_FIELDS = new Map([
  ["foreground", "foreground"],
  ["background", "background"],
  ["cursor", "cursor"],
  ["black", "black"],
  ["red", "red"],
  ["green", "green"],
  ["yellow", "yellow"],
  ["blue", "blue"],
  ["magenta", "magenta"],
  ["cyan", "cyan"],
  ["white", "white"],
  ["brightBlack", "bright_black"],
  ["brightRed", "bright_red"],
  ["brightGreen", "bright_green"],
  ["brightYellow", "bright_yellow"],
  ["brightBlue", "bright_blue"],
  ["brightMagenta", "bright_magenta"],
  ["brightCyan", "bright_cyan"],
  ["brightWhite", "bright_white"],
]);

export function resolveTimeout(
  cls: TimeoutClass,
  callTimeout?: number,
  options: { timeouts?: Timeouts } = {},
): number | undefined {
  if (callTimeout !== undefined && callTimeout !== null) {
    return callTimeout;
  }
  const scoped = options.timeouts?.[cls];
  if (scoped !== undefined && scoped !== null) {
    return scoped;
  }
  return undefined;
}

export function backendPayload(backend?: unknown): Backend | undefined {
  if (backend === undefined || backend === null) {
    return undefined;
  }
  if (typeof backend !== "string") {
    throw new TypeError("backend must be a string");
  }
  const normalized = backend.trim().toLowerCase();
  if ((BACKENDS as readonly string[]).includes(normalized)) {
    return normalized as Backend;
  }
  throw new TypeError(
    `unknown backend "${backend}"; expected one of ${BACKENDS.join(", ")}`,
  );
}

export function timeoutsPayload(
  timeouts?: Timeouts,
): Record<TimeoutClass, number> | undefined {
  if (!timeouts) {
    return undefined;
  }
  assertTimeoutClasses(timeouts);
  const out: Partial<Record<TimeoutClass, number>> = {};
  let any = false;
  for (const cls of TIMEOUT_CLASSES) {
    const value = timeouts[cls];
    if (value !== undefined && value !== null) {
      out[cls] = value;
      any = true;
