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
  if (process.env.TERMPROBE__NATIVE_LIBRARY) return process.env.TERMPROBE__NATIVE_LIBRARY;
  const platform = process.platform;
  const arch = process.arch;
  const libc = platform === "linux"
    ? (process.report.getReport().header.glibcVersionRuntime ? "gnu" : "musl")
    : undefined;
  const tag = [platform, arch, platform === "win32" ? "msvc" : libc].filter(Boolean).join("-");
  const extension = { darwin: "dylib", linux: "so", win32: "dll" }[platform];
  if (!extension) throw new Error(`unsupported native platform: ${platform}`);
  const local = fileURLToPath(new URL(`termprobe.${tag}.${extension}`, import.meta.url));
  if (existsSync(local)) return local;
  return createRequire(import.meta.url).resolve(`@microsoft/termprobe-${tag}`);
}

const path = libraryPath();
const library = koffi.load(path);
let platformLibrary;
if (process.platform === "win32") {
  // Rust thread-local destructors can outlive the JavaScript environment.
  platformLibrary = koffi.load("kernel32.dll");
  const pin = platformLibrary.func("int32_t __stdcall GetModuleHandleExW(uint32_t flags, void *address, _Out_ void **module)");
  const GET_MODULE_HANDLE_EX_FLAG_PIN = 1;
  const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS = 4;
  if (!pin(
    GET_MODULE_HANDLE_EX_FLAG_PIN | GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    library.symbol("tui_abi_version"),
    [null],
  )) {
