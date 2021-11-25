import { isMainThread } from "node:worker_threads";
import { InternalError, UsageError, makeError } from "./errors.js";
import type { ErrorKind, TermprobeError } from "./errors.js";
import type { FailureArtifactRef, FailureDetails } from "./types.js";
import type {
  AutomaticRecordingOptions,
  TraceOptions,
  BellEvent,
  Cell,
  ClipboardWaitOptions,
  Cursor,
  Viewport,
  EffectiveTimeouts,
  FailureArtifactOptions,
  LocatorExpression,
  LocatorNode,
  LocatorStyle,
  MouseClickOptions,
  OpenOptions,
  OpenResult,
  PackedScreen,
  RecordingOptions,
  RunOptions,
  ScreenshotOptions,
  Size,
  SnapshotOptions,
  State,
  TextMatch,
  Timeouts,
  TitleOptions,
} from "../native/index.js";

type NativeBinding = typeof import("../native/index.js");
type NativeSessionHandle = InstanceType<NativeBinding["NativeSession"]>;
type RuntimeOpenOptions = Omit<OpenOptions, "shell" | "backend"> & {
  shell?: string;
  backend?: string;
};
type RuntimeRunOptions = Omit<RunOptions, "backend"> & { backend?: string };
type RuntimeRecordingOptions = Omit<RecordingOptions, "format"> & {
  format?: "apng" | "gif" | "mp4" | "cast";
};
export type RuntimeLocatorNode = Omit<LocatorNode, "kind" | "direction"> & {
  kind: "text" | "style" | "link" | "and" | "or" | "filter";
  direction?: "within" | "after" | "before";
};
export type RuntimeLocatorExpression = { nodes: RuntimeLocatorNode[]; root: number };
export type RuntimeLocatorStyle = LocatorStyle;

const ERROR_PREFIX = "__termprobe_native_error__:";
const USAGE_NAPI_CODES = new Set([
  "InvalidArg",
  "ObjectExpected",
  "StringExpected",
  "NameExpected",
  "FunctionExpected",
  "NumberExpected",
  "BooleanExpected",
  "ArrayExpected",
  "BigintExpected",
  "DateExpected",
  "ArrayBufferExpected",
  "DetachableArraybufferExpected",
]);

let bindingPromise: Promise<NativeBinding> | undefined;
let cachedBinding: NativeBinding | undefined;
let exitHookInstalled = false;

function installExitHook(): void {
  // The native registry is process-wide, but workers have their own exit events.
  if (!isMainThread || exitHookInstalled) {
    return;
  }
  exitHookInstalled = true;
  process.once("exit", () => {
    try {
      cachedBinding?.closeAllSync();
    } catch {}
  });
}

async function importBinding(): Promise<NativeBinding> {
  try {
    const binding = await import("../native/index.js");
    cachedBinding = binding;
    installExitHook();
    return binding;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(
      "failed to load the @microsoft/termprobe native library: " +
        `${message}. Build it with \`npm run build:native\` (requires a Rust ` +
        "toolchain), or install a matching prebuilt platform package.",
      { cause: error },
    );
  }
}

async function loadBinding(): Promise<NativeBinding> {
  if (!bindingPromise) {
    bindingPromise = importBinding();
  }
  try {
    return await bindingPromise;
  } catch (error) {
    bindingPromise = undefined;
    throw error;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

interface NativeErrorEnvelope {
  kind: ErrorKind;
  message: string;
  details?: FailureDetails;
  artifact?: FailureArtifactRef;
}

function isNativeErrorEnvelope(value: unknown): value is NativeErrorEnvelope {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    (value.kind === "assertion" ||
      value.kind === "usage" ||
      value.kind === "no_session" ||
      value.kind === "internal") &&
    "message" in value &&
    typeof value.message === "string"
  );
}

function attachStructuredFailure(
  mapped: ReturnType<typeof makeError>,
  envelope: NativeErrorEnvelope,
): void {
  Object.defineProperties(mapped, {
    details: {
      configurable: true,
      enumerable: true,
      value: envelope.details,
    },
    artifact: {
      configurable: true,
      enumerable: true,
      value: envelope.artifact,
    },
  });
}

export function mapNativeError(error: unknown): TermprobeError {
  const message = errorMessage(error);
  const encodedAt = message.indexOf(ERROR_PREFIX);
  if (encodedAt >= 0) {
    const encoded = message.slice(encodedAt + ERROR_PREFIX.length);
    try {
      const envelope: unknown = JSON.parse(encoded);
      if (!isNativeErrorEnvelope(envelope)) {
        throw new TypeError("native error envelope has an invalid shape");
      }
      const mapped = makeError(envelope.kind, envelope.message);
      attachStructuredFailure(mapped, envelope);
      Object.defineProperty(mapped, "cause", {
        configurable: true,
        value: error,
      });
      return mapped;
    } catch (parseError) {
      const detail =
        parseError instanceof Error ? parseError.message : String(parseError);
      const mapped = new InternalError(`malformed native error envelope: ${detail}`);
      Object.defineProperty(mapped, "cause", {
