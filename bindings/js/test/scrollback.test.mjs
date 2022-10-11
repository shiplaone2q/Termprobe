import assert from "node:assert/strict";
import { test } from "node:test";
import { Termprobe, UsageError, NoSessionError } from "../dist/index.js";
import { NativeSession } from "../native/index.js";
import koffi from "koffi";
import { types } from "../native/abi.js";
import { createAdapter } from "../native/adapter.js";

test("viewport results preserve safe counts and reject lost precision", async () => {
  const result = Buffer.alloc(koffi.sizeof(types.TuiResult));
  let released = 0;
  const { NativeSession: TestSession } = createAdapter({
    tui_get_viewport: { async(_session, callback) { callback(null, result); } },
    tui_result_free() { released++; },
  });
  const terminal = new TestSession("viewport-width");
  for (const count of [0, Number.MAX_SAFE_INTEGER, 9007199254740992n]) {
    koffi.encode(result, types.TuiResult, { viewport: { offset: count, history: count } });
    if (typeof count === "bigint") {
      await assert.rejects(terminal.getViewport(), /safe integer range/);
    } else {
      assert.deepEqual(await terminal.getViewport(), { offset: count, history: count });
    }
  }
  assert.equal(released, 3);
