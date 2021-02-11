import assert from "node:assert/strict";
import { test } from "node:test";

import { UsageError, uniqueSession } from "../dist/index.js";
import { NativeRuntime } from "../dist/native.js";
import {
  closeAllTracked,
  createTerminal,
  defaultShell,
  resetTerminalDefaults,
  setTerminalDefaults,
  terminalSnapshot,
  trackTerminal,
  trackedCount,
  untrackTerminal,
  withTerminal,
} from "../dist/test/index.js";

test("terminalSnapshot trims trailing whitespace per line", () => {
  assert.equal(terminalSnapshot("a  \nb\t\nc"), "a\nb\nc");
});

test("terminalSnapshot drops trailing blank lines but keeps leading/interior", () => {
  assert.equal(terminalSnapshot("\nhi\n\nbye\n   \n\n"), "\nhi\n\nbye");
  assert.equal(terminalSnapshot("only   \n\n\n"), "only");
  assert.equal(terminalSnapshot(""), "");
  assert.equal(terminalSnapshot("\n\n"), "");
});

test("terminalSnapshot normalises carriage returns at line ends", () => {
  assert.equal(terminalSnapshot("a\r\nb\r\n"), "a\nb");
});

test("defaultShell is platform-aware", () => {
  if (process.platform === "win32") {
    assert.equal(defaultShell, "powershell");
  } else if (process.platform === "darwin") {
    assert.equal(defaultShell, "zsh");
  } else {
    assert.equal(defaultShell, "bash");
  }
});

test("uniqueSession has the documented shape", () => {
  const name = uniqueSession();
  assert.match(name, /^termprobe-\d+-[a-z0-9]+-\d+$/);
});

test("uniqueSession sanitizes unsafe characters", () => {
  const name = uniqueSession("a b/c!");
  assert.match(name, /^a-b-c-/);
  assert.match(name, /^[A-Za-z0-9_-]+$/);
});

test("uniqueSession is unique across calls and capped at 64 chars", () => {
  assert.notEqual(uniqueSession(), uniqueSession());
  const long = uniqueSession("x".repeat(200));
  assert.ok(long.length <= 64, `expected <= 64, got ${long.length}`);
  assert.match(long, /^[A-Za-z0-9_-]+$/);
});

test("closeAllTracked closes and forgets every tracked terminal", async () => {
  await closeAllTracked();
  assert.equal(trackedCount(), 0);
  const closed = [];
  const makeStub = (id) => ({
    async closeQuiet() {
      closed.push(id);
    },
  });
  const a = makeStub("a");
  const b = makeStub("b");
  trackTerminal(a);
  trackTerminal(b);
  assert.equal(trackedCount(), 2);
  await closeAllTracked();
