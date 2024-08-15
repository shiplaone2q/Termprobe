import { test } from "node:test";
import assert from "node:assert/strict";
import { attachment, fixture } from "./fixture.mjs";
import { parseReport, cellAt, selection, visibleActions, inspectCell } from "../src/model.js";
import { initialState, update } from "../src/state.js";
import { build } from "esbuild";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import { sveltePlugin } from "../svelte-plugin.mjs";

const parse = (data = fixture()) => parseReport(JSON.stringify(data));
function freeze(value) {
  if (!value || typeof value !== "object" || ArrayBuffer.isView(value)) return value;
  for (const child of Object.values(value)) freeze(child);
  return Object.freeze(value);
}

test("normalization exposes display models, preserves exact input, and never guesses a missing screen", async () => {
  const model = parse();
  assert.equal(model.title, "termprobe: deployment-wizard");
  assert.equal(model.frames[0].viewBox, "7 11 200 80");
  assert.equal(model.actions[0].frameIndex, undefined);
  assert.equal(model.actions[3].label, '"DEPLOY staging"');
  assert.equal(model.actions[3].properties["Bytes (hex)"], "41 0d");
  assert.deepEqual(model.actions.slice(1, 3).map((action) => action.frameIndex), [0, 0]);
  assert.equal(model.failureActionId, 6);
  assert.equal(model.mismatches.length, 2);
  assert.ok(model.mismatches.every((entry) => entry.stage === 1 && entry.x === 0 && entry.y === 0));
  assert.equal(model.result.locator, 'getByText("A").getByStyle({ foreground: "2" })');
  assert.deepEqual(new Uint8Array(await model.attachments[0].blob.arrayBuffer()), new Uint8Array(Buffer.from(fixture().attachments[0].data, "base64")));
});

test("optional histories and metadata normalize to explicit empty or unavailable values", () => {
  const data = fixture();
  delete data.details.recent_operations;
  delete data.details.runtime;
  delete data.details.terminal;
  delete data.details.locator;
  delete data.details.process;
  data.timeline.frames = [];
  const model = parse(data);
  assert.deepEqual(model.actions, []);
  assert.deepEqual(model.mismatches, []);
  assert.equal(model.failureFrameIndex, undefined);
  assert.equal(model.metadata.timeouts.Defaults, "Not captured");
  assert.equal(selection(model, initialState(model)).frame, undefined);
});

test("the result never borrows an unrelated failure on the same screen", () => {
  const data = fixture();
  data.details.recent_operations.push({
    ...data.details.recent_operations.at(-1), sequence: 7, name: "unrelated assertion",
  });
  assert.equal(parse(data).failureActionId, 6);
  data.details.operation.name = "test";
  data.details.reason = "test_failed";
  delete data.details.locator;
  const model = parse(data);
  assert.equal(model.failureActionId, undefined);
  assert.equal(model.result.locator, undefined);
  const state = initialState(model);
  assert.equal(selection(model, state).isFailure, true);
  assert.equal(selection(model, state).action, undefined);
});

for (const [name, change, error] of [
  ["unsupported report version", (data) => { data.details.schema_version = 2; }, /report.details.schema_version/],
  ["unsupported timeline version", (data) => { data.timeline.schema_version = 2; }, /report.timeline.schema_version/],
  ["missing frame text", (data) => { delete data.timeline.frames[0].text; }, /frames\[0\].text/],
  ["invalid geometry", (data) => { data.timeline.geometry.cell_width = 0; }, /geometry.cell_width/],
  ["invalid dimensions", (data) => { data.timeline.frames[0].size.cols = 1.5; }, /size.cols/],
  ["invalid grid", (data) => { data.timeline.frames[0].grid[0][0] = 999; }, /invalid cell grid/],
  ["duplicate frame IDs", (data) => { data.timeline.frames[1].sequence = 1; }, /Duplicate screen sequence/],
  ["duplicate operation IDs", (data) => { data.details.recent_operations[1].sequence = 1; }, /Duplicate action sequence/],
  ["invalid operation time", (data) => { data.details.recent_operations[0].ended_ms = 0; }, /end precedes start/],
  ["invalid expectation", (data) => { data.details.recent_operations[0].expectation.kind = "unknown"; }, /expectation.kind/],
  ["invalid input variant", (data) => { data.details.recent_operations[3].input.arguments.kind = "unknown"; }, /arguments.kind/],
  ["invalid input bytes", (data) => { data.details.recent_operations[3].input.sent_bytes = [256]; }, /sent_bytes\[0\]/],
  ["invalid locator kind", (data) => { data.details.recent_operations[1].expectation.query.selector.kind = "unknown"; }, /selector.kind/],
  ["invalid cell color", (data) => { data.timeline.frames[0].cells[0].fg = {}; }, /cells\[0\].fg/],
  ["invalid base64", (data) => { data.attachments[0].data = "????"; }, /invalid base64/],
