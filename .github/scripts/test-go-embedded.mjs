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
