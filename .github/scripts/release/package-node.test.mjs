import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const script = fileURLToPath(new URL("./package-node.mjs", import.meta.url));
const { nativeTargets } = JSON.parse(fs.readFileSync(new URL("../../../bindings/js/package.json", import.meta.url), "utf8"));
const targets = Object.values(nativeTargets);
const library = (platform) => `termprobe.${platform}.${platform.startsWith("darwin") ? "dylib" : platform.startsWith("win32") ? "dll" : "so"}`;

function fixture(t) {
  const target = path.resolve("target");
  fs.mkdirSync(target, { recursive: true });
  const directory = fs.mkdtempSync(path.join(target, "package-node-test-"));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const bindings = path.join(directory, "bindings/js");
