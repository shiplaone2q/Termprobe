import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { createFixture } from "./release-fixtures.mjs";
import { verifyGoVersion } from "./verify-go-version.mjs";

test("release checks require every Go installation example to match the tag", t => {
  const fixture = createFixture(t);
  const files = ["README.md", "bindings/go/README.md"];
  const current = "go get github.com/microsoft/termprobe/bindings/go@v1.2.3";
  for (const name of files) fs.writeFileSync(path.join(fixture.root, name), current);
  verifyGoVersion(fixture.root, "1.2.3");
  for (const name of files) {
    const filename = path.join(fixture.root, name);
