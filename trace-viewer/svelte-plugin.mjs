import { readFile } from "node:fs/promises";
import path from "node:path";
import { compile } from "svelte/compiler";

export function sveltePlugin(generate = "client") {
  const styles = new Map();
  return {
    name: "svelte-5",
    setup(build) {
      build.onLoad({ filter: /\.svelte$/ }, async ({ path: filename }) => {
        const source = (await readFile(filename, "utf8")).replace(/\r\n/g, "\n");
        const compiled = compile(source, {
          filename, generate, runes: true, dev: false, css: "external", discloseVersion: false,
          cssHash: ({ hash, css }) => `svelte-${hash(css)}`,
        });
        if (compiled.warnings.length) {
