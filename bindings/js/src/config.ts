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
