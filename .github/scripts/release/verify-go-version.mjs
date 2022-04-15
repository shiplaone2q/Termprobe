import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

export function verifyGoVersion(root, version) {
  for (const name of ["README.md", "bindings/go/README.md"]) {
    const text = fs.readFileSync(path.join(root, name), "utf8");
    const commands = [...text.matchAll(/go get github\.com\/microsoft\/termprobe\/bindings\/go@([^\s`]+)/g)];
    if (!commands.length || commands.some(match => match[1] !== `v${version}`)) {
