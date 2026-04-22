import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { createFixture, root, scripts, targets } from "./release-fixtures.mjs";
import { directoryEntries } from "./package-native.mjs";

function publicationFixture(t) {
  const fixture = createFixture(t);
  const remote = path.join(fixture.root, "remote.git");
  const workflow = fs.readFileSync(path.join(root, ".github/workflows/release.yml"), "utf8");
  const step = workflow.split("      - name: Publish immutable Go module tag\n")[1];
  const script = step.split("        run: |\n")[1].split("\n  release:")[0].replace(/^ {10}/gm, "");
  const filename = path.join(fixture.root, ".github/scripts/release/package-native.mjs");
  fs.mkdirSync(path.dirname(filename), { recursive: true });
  fs.copyFileSync(path.join(scripts, "package-native.mjs"), filename);
  function git(args, cwd = fixture.root, encoding = "utf8") {
    return execFileSync("git", [...(cwd === remote ? ["--git-dir", remote] : []), ...args], {
      cwd, encoding, stdio: ["ignore", "pipe", "pipe"],
    });
  }
  git(["init", "-q", "-b", "main"]);
  const identity = script.split("\n").filter(line => line.startsWith("git config user.")).join("\n");
  assert.ok(identity, "workflow automation identity is missing");
  execFileSync("bash", ["-ec", identity], { cwd: fixture.root });
  git(["config", "commit.gpgsign", "false"]);
  git(["add", "bindings", "LICENSE", ".github"]);
  git(["commit", "-q", "-s", "-m", "Source release fixture"]);
  fixture.revision = git(["rev-parse", "HEAD"]).trim();
  git(["tag", "1.2.3"]);
  git(["tag", "1.2.4"]);
  git(["init", "-q", "--bare", "-b", "main", remote]);
  git(["remote", "add", "origin", remote]);
  git(["push", "-q", "origin", "main", "--tags"]);
  for (const target of Object.keys(targets)) fixture.writeNative(target);
  fixture.assemble();
  return {
    fixture, remote, git,
    ref(name) { return git(["rev-parse", name], remote).trim(); },
    publish(version = "1.2.3") {
      const workspace = fs.mkdtempSync(path.join(fixture.root, "publisher-"));
      git(["clone", "-q", "--no-checkout", remote, workspace]);
      git(["checkout", "-q", "--detach", version], workspace);
      git(["config", "commit.gpgsign", "false"], workspace);
      fs.cpSync(fixture.output, path.join(workspace, "target/go-package"), { recursive: true });
      fs.symlinkSync(path.join(scripts, "../node_modules"), path.join(workspace, "node_modules"), "junction");
