import assert from "node:assert/strict";
import { once } from "node:events";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { test } from "node:test";
import { Worker } from "node:worker_threads";

import {
  ExpectationError,
  InternalError,
  NoSessionError,
  Termprobe,
  UsageError,
  closeAll,
  getRecording,
  sessions,
  uniqueSession,
} from "../dist/index.js";
import { NativeRuntime } from "../dist/native.js";
import { withTerminal } from "../dist/test/index.js";

const shell = process.platform === "win32" ? "pwsh" : undefined;
const evalArgs =
  typeof globalThis.Deno === "undefined"
    ? ["-e", "console.log('ready'); setInterval(() => {}, 1000)"]
    : ["eval", "console.log('ready'); setInterval(() => {}, 1000)"];
const twoBellsCommand =
  process.platform === "win32"
    ? "[Console]::Out.Write([char]7); [Console]::Out.Write([char]7)"
    : "printf '\\a\\a'";
const delayedBellCommand =
  process.platform === "win32"
    ? "Start-Sleep -Seconds 1; [Console]::Out.Write([char]7)"
    : "sleep 1; printf '\\a'";
const clipboardCommand = (base64) =>
  process.platform === "win32"
    ? `[Console]::Out.Write(([char]27).ToString() + ']52;c;${base64}' + ([char]7).ToString())`
    : `printf '\\033]52;c;${base64}\\a'`;
const nonzeroExitArgs =
  typeof globalThis.Deno === "undefined"
    ? ["-e", "process.exit(7)"]
    : ["eval", "Deno.exit(7)"];
const runtimeScript = (script) =>
  typeof globalThis.Deno === "undefined"
    ? ["-e", script]
    : ["eval", `import process from "node:process"; ${script}`];

test("echo roundtrip drives a real session", async () => {
  await withTerminal({ shell }, async (su) => {
    await su.submit("echo hello-sdk");
    await su.waitCommand();
    await su.getByText("hello-sdk").first().expect();

    // A command finishes before the shell draws its next prompt. Wait for the
    // prompt-end marker so these separate cursor reads cannot straddle that
    // redraw and disagree by the prompt width.
    await su.waitReady();
    const state = await su.state();
    assert.deepEqual(await su.getCursor(), state.cursor);
    assert.ok(state.cols > 0);
    assert.match(await su.text(), /hello-sdk/);
    assert.equal(typeof (await su.getCwd()), "string");
    assert.deepEqual(await su.getSize(), { cols: state.cols, rows: state.rows });

    await su.resize(92, 26);
    assert.deepEqual(await su.getSize(), { cols: 92, rows: 26 });
    assert.ok((await su.cells(0, 0, 92, 26)).length > 0);
    assert.match(await su.screenshot(), /hello-sdk/);
    await assert.rejects(() => su.screenshot(null, { zoom: 0.5 }), /requires a path/);

    await su.write("echo typed-write");
    await su.keyboard.press("Enter");
    await su.getByText("typed-write").wait();
    await su.waitCommand();
    await su.type("echo typed-type");
    await su.keyboard.press("Enter");
    await su.getByText("typed-type").wait();
    await su.waitCommand();
  });
});

test("restart recreates a named session with an open result", async () => {
  const su = new Termprobe(uniqueSession("restart-binding"));
  try {
    const opened = await su.run(process.execPath, evalArgs);
    await su.getByText("ready").wait({ timeout: 2000 });
    const restarted = await su.restart({ gracefulTimeout: 0 });
    assert.deepEqual(Object.keys(restarted).sort(), Object.keys(opened).sort());
    assert.equal(restarted.session, su.session);
    await su.getByText("ready").wait({ timeout: 2000 });
    await su.close();
    await assert.rejects(
      () => su.restart({ gracefulTimeout: 0 }),
      (error) => error instanceof NoSessionError,
    );
  } finally {
    await su.closeQuiet();
  }
});

test("invalid open and run options with retries preserve the existing session", async () => {
  const terminal = new Termprobe(uniqueSession("invalid-spawn-preserves"));
  try {
    const opened = await terminal.run(process.execPath, evalArgs);
    await terminal.getByText("ready").wait({ timeout: 5000 });
    for (const options of [{ cols: -1 }, { timeouts: { ready: -1 } }]) {
      const invalid = { ...options, retries: 2, restart: true };
      for (const action of [
        () => terminal.open(invalid),
        () => terminal.run(process.execPath, evalArgs, invalid),
      ]) {
        await assert.rejects(action, UsageError);
        assert.ok((await sessions()).includes(terminal.session));
        await terminal.getByText("ready").expect({ timeout: 0 });
        const reused = await terminal.run(process.execPath, evalArgs);
        assert.equal(reused.shell_pid, opened.shell_pid);
      }
    }
  } finally {
    await terminal.closeQuiet();
  }
});

test("worker exit leaves terminals owned by the main thread open", {
  skip: !!process.versions.bun || typeof globalThis.Deno !== "undefined",
  timeout: 20_000,
}, async (t) => {
  const terminal = new Termprobe(uniqueSession("worker-owner"));
  try {
    await terminal.run(process.execPath, evalArgs);
    await terminal.getByText("ready").wait({ timeout: 5000 });
    for (const explicitExit of [false, true]) {
      const worker = new Worker(`
        const assert = require("node:assert/strict");
        const { workerData } = require("node:worker_threads");
        (async () => {
          const sdk = await import(workerData.url);
          assert.ok((await sdk.sessions()).includes(workerData.session));
          const terminal = new sdk.Termprobe(workerData.session);
          assert.deepEqual(await terminal.getSize(), { cols: 80, rows: 30 });
          const reads = Array.from({ length: 300 }, () => terminal.getSize());
          if (workerData.explicitExit) process.exit(0);
          await Promise.all(reads);
        })().catch((error) => { throw error; });
      `, {
        eval: true,
        workerData: {
          url: new URL("../dist/index.js", import.meta.url).href,
          session: terminal.session,
          explicitExit,
        },
      });
      t.after(() => worker.terminate());
      const [code] = await once(worker, "exit");
      assert.equal(code, 0);
      assert.ok((await sessions()).includes(terminal.session));
      await terminal.getByText("ready").expect({ timeout: 0 });
    }
  } finally {
    await terminal.closeQuiet();
  }
});

test("restart preserves failure-artifact recording without trace options", async () => {
  const root = mkdtempSync(join(tmpdir(), "termprobe-restart-artifacts-"));
  const su = new Termprobe(uniqueSession("restart-artifacts"), {
    artifacts: { dir: root, onFailure: "text", includeRecording: true },
  });
  try {
    const opened = await su.run(process.execPath, evalArgs);
    assert.ok(opened.recording);
    const restarted = await su.restart({ gracefulTimeout: 0 });
    assert.ok(restarted.recording);
    await su.getByText("ready").wait({ timeout: 5000 });
    let failure;
    await assert.rejects(su.getByText("missing restart marker").expect({ timeout: 0 }), (error) => {
      failure = error;
      return error instanceof ExpectationError;
    });
    assert.ok(failure.artifact.recording);
    assert.match(await readFile(failure.artifact.recording, "utf8"), /ready/);
  } finally {
    await su.closeQuiet();
    rmSync(root, { recursive: true, force: true });
  }
});

test("bell state, waits, and expectations stay consistent", async () => {
  const su = Termprobe.ephemeral("bell-events");

  try {
    await su.open({ shell });
    await su.submit(twoBellsCommand);
    await su.expectBellCount(2, { timeout: 5000 });
    await su.waitCommand();

    const initialState = await su.state();
    assert.equal(initialState.bell_count, 2);
    const initialEvents = await su.getBellEvents();
    assert.deepEqual(
      initialEvents.map((event) => event.sequence),
      [1, 2],
    );
    assert.ok(initialEvents[1].elapsed_ms >= initialEvents[0].elapsed_ms);
    assert.equal(await su.getBellCount(), 2);
    assert.equal(await su.getBellCount(), 2);

    await su.submit(delayedBellCommand);
    await su.waitBell({ timeout: 5000 });
    await su.expectBellCount(3);
    assert.equal(await su.getBellCount(), 3);
    const finalState = await su.state();
    assert.equal(finalState.bell_count, 3);
    const finalEvents = await su.getBellEvents();
    assert.deepEqual(
      finalEvents.map((event) => event.sequence),
      [1, 2, 3],
    );
    assert.ok(finalEvents[2].elapsed_ms >= finalEvents[1].elapsed_ms);
  } finally {
    await su.closeQuiet();
  }
});

test("clipboard getter and change wait stay consistent", async () => {
  await withTerminal({ shell }, async (su) => {
    assert.equal(await su.getClipboard(), "");

    await su.submit(clipboardCommand("Y2hhbmdlZA=="));
    await su.waitCommand();
    await su.waitClipboard({ timeout: 5000 });
    assert.equal(await su.getClipboard(), "changed");

    await su.submit(clipboardCommand("cHJlZml4LXJlYWR5LTQy"));
    await su.waitCommand();
    await su.waitClipboard("ready", { timeout: 5000 });
    await assert.rejects(
      () => su.waitClipboard("ready", { regex: true, timeout: 100 }),
      (error) => error instanceof UsageError,
    );

    await su.submit(clipboardCommand("YnVpbGQtMTIz"));
    await su.waitCommand();
    await su.waitClipboard(/^BUILD-[0-9]+$/i, { timeout: 5000 });

    await su.submit(clipboardCommand("eApmb28="));
    await su.waitCommand();
    await assert.rejects(
      () => su.waitClipboard(/foo/my, { timeout: 100 }),
      (error) => error instanceof ExpectationError,
    );
    await su.submit(clipboardCommand("Zm9vCng="));
    await su.waitCommand();
    await su.waitClipboard(/foo/my, { timeout: 5000 });

    await su.submit(clipboardCommand("8J+YgA=="));
    await su.waitCommand();
    await assert.rejects(
      () => su.waitClipboard(/^..$/u, { timeout: 100 }),
      (error) => error instanceof ExpectationError,
    );
    await su.waitClipboard(/^..$/, { timeout: 5000 });

    await su.submit(clipboardCommand("eGZvbw=="));
    await su.waitCommand();
    const sticky = /foo/y;
    sticky.lastIndex = 1;
    await su.waitClipboard(sticky, { timeout: 5000 });
    assert.equal(sticky.lastIndex, 1);

    const globalAfterMatch = /foo/g;
    globalAfterMatch.lastIndex = 2;
    await assert.rejects(
      () => su.waitClipboard(globalAfterMatch, { timeout: 100 }),
      (error) => error instanceof ExpectationError,
    );
    assert.equal(globalAfterMatch.lastIndex, 2);
  });
});

test("xterm.js clipboard rejects as InternalError", async () => {
  const su = new Termprobe(uniqueSession("xtermjs-clipboard"), {
    backend: "xtermjs",
  });
  try {
    await su.open({ shell });
    await assert.rejects(
      () => su.getClipboard(),
      (error) =>
        error instanceof InternalError && error.message.includes("unavailable"),
    );
  } finally {
    await su.closeQuiet();
  }
});

test("recording API writes an asciicast file", async () => {
  const root = mkdtempSync(join(tmpdir(), "termprobe-recording-"));
  const path = join(root, "demo.cast");
  try {
    await withTerminal({ shell }, async (su) => {
      await su.startRecording(path, {
        format: "cast",
        fps: 24,
        speed: 1,
        idleTimeLimit: 2,
      });
      await su.submit("echo sdk-recording");
      await su.waitCommand();
      assert.equal(await su.stopRecording(), path);
    });
    const cast = await readFile(path, "utf8");
    assert.match(cast, /"version":2/);
    assert.match(cast, /sdk-recording/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("trace retention and recording directory are configurable", async () => {
  const root = mkdtempSync(join(tmpdir(), "termprobe-auto-recording-"));
  const disabled = new Termprobe(uniqueSession("recording-disabled"), {
    recording: { directory: root },
    trace: { mode: "off", directory: join(root, "traces") },
  });
  assert.equal((await disabled.open({ shell, waitReady: false })).recording, "");
  await disabled.close();

  const always = new Termprobe(uniqueSession("recording-always"), {
    recording: { directory: root },
    trace: { mode: "on", directory: join(root, "traces") },
  });
