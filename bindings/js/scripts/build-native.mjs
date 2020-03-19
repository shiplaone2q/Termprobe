import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const directory = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const root = resolve(directory, "../..");
const args = process.argv.slice(2);
let debug = false;
let zigbuild = false;
let explicitTarget;
for (let i = 0; i < args.length; i++) {
  const arg = args[i];
  if (arg === "--debug") debug = true;
  else if (arg === "--zig" || arg === "-x") zigbuild = true;
  else if (arg === "--target" || arg === "-t") {
    explicitTarget = args[++i];
    if (!explicitTarget || explicitTarget.startsWith("-")) throw new Error("Missing --target value");
  } else if (arg.startsWith("--target=") && arg.length > "--target=".length) {
    explicitTarget = arg.slice("--target=".length);
  } else {
