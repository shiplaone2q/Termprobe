import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

export function verifyGoVersion(root, version) {
  for (const name of ["README.md", "bindings/go/README.md"]) {
