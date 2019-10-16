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
