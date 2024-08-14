import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";

const root = fileURLToPath(new URL("../../../", import.meta.url));

function build(t, args, host = "x86_64-unknown-linux-gnu") {
  fs.mkdirSync(path.join(root, "target"), { recursive: true });
  const directory = fs.mkdtempSync(path.join(root, "target/release-build-test-"));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const bindings = path.join(directory, "bindings/js");
  fs.mkdirSync(path.join(bindings, "scripts"), { recursive: true });
  for (const name of ["package.json", "scripts/build-native.mjs"]) {
    fs.copyFileSync(path.join(root, "bindings/js", name), path.join(bindings, name));
  }
  const mock = path.join(directory, "mock #%.mjs");
  fs.writeFileSync(mock, `
import childProcess from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { syncBuiltinESMExports } from "node:module";
childProcess.execFileSync = (command, args, options) => {
  if (command === "rustc") return "host: ${host}\\n";
  if (command !== "cargo" && command !== "docker") throw new Error("Unexpected command: " + command);
  fs.writeFileSync(path.join(options.cwd, "invocation.json"), JSON.stringify({
    command, args, rustflags: options.env.RUSTFLAGS, targetDir: options.env.CARGO_TARGET_DIR,
  }));
  const index = args.indexOf("--target");
  const target = command === "docker" ? args.at(-2) : index === -1 ? "" : args[index + 1];
  const profile = command === "docker" ? args.at(-1) : args.includes("--release") ? "release" : "debug";
  const output = path.join(options.cwd, "target", target, profile);
  fs.mkdirSync(output, { recursive: true });
  const library = target.includes("windows") ? "termprobe_ffi.dll"
    : target.includes("apple") ? "libtermprobe_ffi.dylib" : "libtermprobe_ffi.so";
  fs.writeFileSync(path.join(output, library), "native build fixture");
};
syncBuiltinESMExports();
`);
  execFileSync(process.execPath, ["--import", pathToFileURL(mock).href, path.join(bindings, "scripts/build-native.mjs"), ...args], {
    cwd: directory,
    env: { ...process.env, RUSTFLAGS: "-C debuginfo=1", CARGO_TARGET_DIR: "caller-target" },
