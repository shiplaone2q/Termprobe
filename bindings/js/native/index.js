import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import koffi from "koffi";
import { abiVersion, bind } from "./abi.js";
import { createAdapter } from "./adapter.js";

export {
  Backend, Shell, RecordingFormat, SnapshotResult, UnderlineStyle,
  LocatorNodeKind, LocatorStageDirection,
} from "./models.js";

function libraryPath() {
