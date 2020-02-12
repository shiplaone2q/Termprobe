import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { createFixture } from "./release-fixtures.mjs";
import { verifyGoVersion } from "./verify-go-version.mjs";

test("release checks require every Go installation example to match the tag", t => {
  const fixture = createFixture(t);
