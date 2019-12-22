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
