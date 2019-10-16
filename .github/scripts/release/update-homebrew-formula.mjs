import fs from "node:fs";

const version = process.argv[2];
if (!version) {
  throw new Error("Usage: update-homebrew-formula.mjs <version>");
}

const { assets } = JSON.parse(fs.readFileSync(0, "utf8"));
const sha256 = (name) => {
  const digest = assets.find((asset) => asset.name === name)?.digest;
  if (!digest?.startsWith("sha256:")) {
    throw new Error(`Missing SHA-256 digest for ${name}`);
  }
  return digest.slice("sha256:".length);
};
