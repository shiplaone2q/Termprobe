import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { test } from "node:test";

import { Termprobe } from "../dist/index.js";
import {
  ExpectationError,
  InternalError,
  NoSessionError,
  UsageError,
} from "../dist/index.js";
import { NativeRuntime, mapNativeError } from "../dist/native.js";

const ERROR_PREFIX = "__termprobe_native_error__:";

function runWithGc(script) {
  const args = typeof globalThis.Deno !== "undefined"
    ? ["eval", "--v8-flags=--expose-gc", script]
    : typeof globalThis.Bun !== "undefined"
      ? ["-e", script]
      : ["--expose-gc", "--input-type=module", "-e", script];
  const child = spawnSync(process.execPath, args, { encoding: "utf8", env: process.env });
  assert.equal(child.status, 0, child.stderr);
}

test("the native function table retains its library through garbage collection", () => {
  runWithGc(`
    import assert from "node:assert/strict";
    const binding = await import(${JSON.stringify(new URL("../native/index.js", import.meta.url).href)});
    for (let i = 0; i < 10; i++) {
      if (typeof globalThis.Bun !== "undefined") globalThis.Bun.gc(true);
      else globalThis.gc();
      await new Promise(resolve => setTimeout(resolve, 0));
    }
    assert.deepEqual(await binding.sessions(), []);
  `);
});

test("a worker-only native library survives worker and process teardown", {
  skip: !!process.versions.bun || typeof globalThis.Deno !== "undefined",
}, () => {
  const worker = `
    (async () => {
      const { workerData } = await import("node:worker_threads");
      const { Termprobe } = await import(workerData);
      const terminal = Termprobe.ephemeral();
      try {
        await terminal.run(process.execPath, ["-e", "console.log('ready')"]);
        await terminal.getSize();
      } finally {
        await terminal.close();
      }
    })().catch(error => { console.error(error); process.exitCode = 1; });
  `;
  runWithGc(`
    import assert from "node:assert/strict";
    import { once } from "node:events";
    import { Worker } from "node:worker_threads";
    for (let i = 0; i < 3; i++) {
      const worker = new Worker(${JSON.stringify(worker)}, {
        eval: true,
        workerData: ${JSON.stringify(new URL("../dist/index.js", import.meta.url).href)},
      });
      const [code] = await once(worker, "exit");
      assert.equal(code, 0);
    }
  `);
});

test("terminating a worker during a native wait preserves the loaded engine", {
  skip: !!process.versions.bun || typeof globalThis.Deno !== "undefined",
}, () => {
  const worker = `
    (async () => {
      const { parentPort, workerData } = await import("node:worker_threads");
      const binding = await import(workerData.url);
      if (workerData.cleanup) {
        const assert = (await import("node:assert/strict")).default;
        assert.ok((await binding.sessions()).includes("pending-worker"));
        await binding.closeAll();
        return;
      }
      const session = new binding.NativeSession("pending-worker");
      await session.run({ program: process.execPath, args: ["-e", "setInterval(() => {}, 1000)"] });
      session.waitTitle("never-matches", { timeoutMs: 30000 }).catch(() => {});
      parentPort.postMessage("waiting");
    })().catch(error => { console.error(error); process.exitCode = 1; });
  `;
  runWithGc(`
    import assert from "node:assert/strict";
    import { once } from "node:events";
    import { Worker } from "node:worker_threads";
    const options = {
      eval: true,
      workerData: { url: ${JSON.stringify(new URL("../native/index.js", import.meta.url).href)} },
    };
    const worker = new Worker(${JSON.stringify(worker)}, options);
    assert.deepEqual(await once(worker, "message"), ["waiting"]);
    assert.equal(await worker.terminate(), 1);
    const cleanup = new Worker(${JSON.stringify(worker)}, {
      ...options, workerData: { ...options.workerData, cleanup: true },
    });
    assert.deepEqual(await once(cleanup, "exit"), [0]);
  `);
});

test("nested input buffers remain live during native start and marshaling", () => {
  runWithGc(`
    import assert from "node:assert/strict";
    import koffi from "koffi";
    import { types } from ${JSON.stringify(new URL("../native/abi.js", import.meta.url).href)};
    import { createAdapter } from ${JSON.stringify(new URL("../native/adapter.js", import.meta.url).href)};
    const expected = "nested-owned-buffer-".repeat(16000);
    const read = value => Buffer.from(koffi.decode(value.data, "uint8_t", value.len)).toString();
    let calls = 0;
    const { NativeSession: TestSession } = createAdapter({
      tui_run_start(context, options, program, argv, argc) {
        for (let i = 0; i < 4; i++) {
          if (typeof globalThis.Bun !== "undefined") globalThis.Bun.gc(true);
          else globalThis.gc();
          Buffer.alloc(400000, 0x58);
        }
        const arg = koffi.decode(argv, types.TuiString, argc)[0];
        const pair = koffi.decode(options.env, types.TuiPair, options.env_len)[0];
        assert.ok(read(arg) === expected, "argument buffer changed during native start");
        assert.ok(read(pair.value) === expected, "environment buffer changed during native start");
        calls++;
