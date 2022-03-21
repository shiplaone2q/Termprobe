import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { gzipSync } from "node:zlib";
import { Header } from "tar";
import * as packages from "./package-native.mjs";
import * as elf from "./check-native-elf.mjs";
import { createFixture, libraryFixture, targets } from "./release-fixtures.mjs";

test("native CLI creates deterministic archives for every platform", async t => {
  const fixture = createFixture(t);
  for (const target of Object.keys(targets)) {
    const library = path.join(fixture.root, packages.libraryName(target));
    fs.writeFileSync(library, libraryFixture(target));
    const args = ["--library", library, "--target", target];
    fixture.run("native", args);
    const filename = path.join(fixture.root, "target/native-packages", packages.nativeFilename("1.2.3", target));
    const first = fs.readFileSync(filename);
    fixture.run("native", args);
    assert.deepEqual(fs.readFileSync(filename), first);
    assert.deepEqual(first.subarray(4, 8), Buffer.alloc(4));
    assert.equal(fs.readFileSync(`${filename}.sha256`, "utf8"), `${packages.sha256(first)}  ${path.basename(filename)}\n`);
    assert.equal((await packages.readNative(first, "1.2.3", fixture.revision, target)).target, target);
    const members = await packages.readArchive(first);
    assert.equal(members.length, 3);
    for (const member of members) {
      assert.deepEqual([member.uid, member.gid, +member.mtime, member.mode], [0, 0, 0, 0o644]);
    }
  }
});

test("wrong platform and truncated library headers are rejected", t => {
  const fixture = createFixture(t);
  const filename = path.join(fixture.root, "wrong.so");
  fs.writeFileSync(filename, libraryFixture("aarch64-apple-darwin"));
  fixture.run("native", ["--library", filename, "--target", "aarch64-unknown-linux-gnu"], false);
  assert.equal(fs.existsSync(path.join(fixture.root, "target/native-packages")), false);
  for (const target of Object.keys(targets)) {
    assert.throws(() => packages.validateLibrary(Buffer.alloc(0), target), /format or architecture/);
  }
});

test("module assembly includes all targets without changing source", async t => {
  const fixture = createFixture(t);
  const before = packages.directoryEntries(fixture.source);
  fixture.assemble();
  assert.deepEqual(packages.directoryEntries(fixture.source), before);
  const contents = packages.directoryEntries(fixture.output);
  assert.deepEqual(await packages.verifyModule(fixture.root, fixture.output), contents);
  assert.deepEqual(contents.get("LICENSE"), fs.readFileSync(path.join(fixture.root, "LICENSE")));
  assert.equal([...contents.keys()].filter(name => /bundled_.*\.go$/.test(name)).length, 5);
  assert.equal([...contents.keys()].filter(name => name.endsWith(".tar.gz")).length, 7);
  const filename = path.join(fixture.root, "termprobe-go-1.2.3.tar.gz");
  assert.ok(fs.existsSync(`${filename}.sha256`));
  const archived = await packages.readArchive(fs.readFileSync(filename));
  assert.deepEqual(new Map(archived.map(entry => [entry.name, entry.data])), contents);
