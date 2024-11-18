import { Termprobe } from "../client.js";
import { IS_MACOS, IS_WINDOWS } from "../config.js";
import { uniqueSession } from "../ephemeral.js";
import { UsageError } from "../errors.js";
import type {
  ArtifactOptions,
  AutomaticRecording,
  TraceOptions,
  Backend,
  ClientOptions,
  Profile,
  Shell,
  SpawnOptions,
  Timeouts,
} from "../types.js";

export type { Profile, Shell } from "../types.js";
export { Termprobe } from "../client.js";

export interface CreateTerminalOptions {
  backend?: Backend;
  shell?: Shell;
  program?: string[];
  cols?: number;
  rows?: number;
  cwd?: string;
  env?: Record<string, string | number | boolean> | [string, string][];
  session?: string;
  prefix?: string;
  retries?: number;
  waitReady?: boolean;
  timeouts?: Timeouts;
  screenHistoryLimit?: number;
  profile?: Profile;
  artifacts?: ArtifactOptions;
  recording?: AutomaticRecording;
  trace?: TraceOptions;
}

let defaults: Partial<CreateTerminalOptions> = {};

export function setTerminalDefaults(values: Partial<CreateTerminalOptions>): void {
  defaults = { ...defaults, ...values };
}

export function resetTerminalDefaults(): void {
  defaults = {};
}

const tracked = new Set<Termprobe>();
let safetyNetInstalled = false;

function installSafetyNet(): void {
  if (safetyNetInstalled) {
    return;
  }
