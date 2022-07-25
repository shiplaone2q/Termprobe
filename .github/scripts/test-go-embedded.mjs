import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const { positionals } = parseArgs({ allowPositionals: true });
if (positionals.length !== 1) throw new Error("Usage: test-go-embedded.mjs <package>");
const packagePath = path.resolve(positionals[0]);
const root = fileURLToPath(new URL("../../", import.meta.url));
const target = path.join(root, "target");
fs.mkdirSync(target, { recursive: true });
const directory = fs.mkdtempSync(path.join(target, "go-consumer-"));
try {
  const goroot = execFileSync("go", ["env", "GOROOT"], { encoding: "utf8" }).trim();
  const go = path.join(goroot, "bin", process.platform === "win32" ? "go.exe" : "go");
  const environment = { ...process.env, CGO_ENABLED: "0", GOWORK: "off", GOTOOLCHAIN: "local", GOROOT: goroot };
  // Windows environment names are case-insensitive, including Path/PATH.
  for (const name of Object.keys(environment)) {
    if (["PATH", "TERMPROBE__NATIVE_LIBRARY", "GOFLAGS", "GOOS", "GOARCH"].includes(name.toUpperCase())) delete environment[name];
  }
  environment.PATH = "";
  for (const args of [
    ["mod", "init", "example.com/termprobe-consumer"],
    ["mod", "edit", `-replace=github.com/microsoft/termprobe/bindings/go=${packagePath}`],
    ["get", "github.com/microsoft/termprobe/bindings/go@v0.0.0"],
    ["build", "-buildvcs=false", "-o", path.join(directory, "smoke.exe"), "github.com/microsoft/termprobe/bindings/go/examples/smoke"],
  ]) {
    execFileSync(go, args, { cwd: directory, env: environment, stdio: "inherit" });
  }
