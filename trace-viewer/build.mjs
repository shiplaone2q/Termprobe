import { build, context } from "esbuild";
import { watchFile, unwatchFile } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { sveltePlugin } from "./svelte-plugin.mjs";

const source = fileURLToPath(new URL(".", import.meta.url));
const destination = fileURLToPath(new URL("../crates/termprobe/assets/trace-viewer/", import.meta.url));
const templatePath = path.join(source, "index.html");
const previewPath = path.join(source, ".preview", "failure.html");
const check = process.argv.includes("--check");
const watch = process.argv.includes("--watch");
const reportArgument = process.argv.indexOf("--report");
const reportPath = reportArgument < 0 ? undefined : process.argv[reportArgument + 1];
if (reportArgument >= 0 && (!reportPath || reportPath.startsWith("--"))) throw new Error("--report requires a saved trace HTML path");
if (check && (watch || reportPath)) throw new Error("--check cannot be combined with --watch or --report");
if (watch && !reportPath) throw new Error("Use npm run dev -- --report <saved trace HTML>");
if (reportPath && path.relative(path.resolve(reportPath), previewPath) === "") {
  throw new Error("Use the original captured report as --report input, not the generated preview");
}

const runtimePackages = ["svelte", "esm-env", "clsx"];
const licenses = await Promise.all(runtimePackages.map(async (name) => {
  const file = name === "svelte" ? "LICENSE.md" : name === "esm-env" ? "LICENSE" : "license";
  const license = (await readFile(path.join(source, "node_modules", name, file), "utf8")).replace(/\r\n/g, "\n").trim();
  return `/*! ${name}\n${license}\n*/`;
}));
const options = {
  absWorkingDir: source, entryPoints: ["src/index.js"], outfile: "report.min.js",
  bundle: true, minify: true, treeShaking: true, charset: "ascii", legalComments: "none",
  target: "es2023", format: "iife", platform: "browser", write: false, metafile: true,
  supported: { "template-literal": false },
  conditions: ["svelte", "browser", "production"], define: { "process.env.NODE_ENV": '"production"' },
  banner: { js: licenses.join("\n") },
  plugins: [sveltePlugin(), {
    name: "standalone-report",
    setup(build) {
      build.onEnd(async (result) => {
        if (result.errors.length) return;
        for (const input of Object.keys(result.metafile.inputs)) {
