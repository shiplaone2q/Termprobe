import { readFile } from "node:fs/promises";
import path from "node:path";
import { compile } from "svelte/compiler";

export function sveltePlugin(generate = "client") {
  const styles = new Map();
  return {
    name: "svelte-5",
    setup(build) {
      build.onLoad({ filter: /\.svelte$/ }, async ({ path: filename }) => {
