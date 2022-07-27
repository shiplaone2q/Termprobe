import { execFileSync } from "node:child_process";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

export function checkVersions(output) {
  const versions = [...output.matchAll(/\bGLIBC_(\d+(?:\.\d+)+)\b/g)].map(match => match[1]);
  if (!versions.length || output.includes("GLIBC_PRIVATE")) throw new Error("Missing or unsupported glibc symbol versions");
  const newer = [...new Set(versions.filter(version => {
    const [major, minor, ...rest] = version.split(".").map(Number);
    return major > 2 || (major === 2 && (minor > 28 || (minor === 28 && rest.length > 0)));
  }))].sort();
  if (newer.length) throw new Error(`Library requires glibc newer than 2.28: ${newer.join(", ")}`);
