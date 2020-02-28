import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const bindingsDirectory = path.resolve("bindings/js");
const nativePackagesDirectory = path.join(bindingsDirectory, "npm");
const outputDirectory = path.resolve("package-artifacts/npm");
const packagePath = path.join(bindingsDirectory, "package.json");
const originalPackage = fs.readFileSync(packagePath, "utf8");
const rootPackage = JSON.parse(originalPackage);
rootPackage.optionalDependencies = {};

const runNpm = (args) => execFileSync("npm", args, { stdio: "inherit" });
