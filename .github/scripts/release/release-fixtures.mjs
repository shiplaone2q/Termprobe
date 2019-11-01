import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as packages from "./package-native.mjs";

export const scripts = fileURLToPath(new URL("./", import.meta.url));
export const root = fileURLToPath(new URL("../../../", import.meta.url));
export const targets = JSON.parse(fs.readFileSync(path.join(root, "bindings/js/package.json"), "utf8")).nativeTargets;

export function libraryFixture(target) {
  const data = Buffer.alloc(128);
  const arm = target.startsWith("aarch64");
  if (target.includes("linux")) {
    data.write("\x7fELF\x02\x01", 0, "binary");
