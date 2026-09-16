import { describeExpectation, describeOperation, json, time } from "./format.js";
import { validateReport } from "./schema.js";

/** @import { TraceModel, TraceFrame, TraceAction, TraceAttachment, FrameCell, ViewerState } from "./types.js" */

/** @param {string} source @returns {TraceModel} */
export function parseReport(source) {
  const data = JSON.parse(source);
  validateReport(data);
  const { details, timeline, explanation } = data;
  const { geometry } = timeline;
  const hasFailure = details.outcome !== "passed";
  /** @type {Map<number, number>} */
  const frameIndex = new Map();
  /** @type {TraceFrame[]} */
  const frames = timeline.frames.map((frame, index) => {
    if (frameIndex.has(frame.sequence)) throw new Error(`Duplicate screen sequence: ${frame.sequence}`);
    frameIndex.set(frame.sequence, index);
    if (frame.last_seen_ms < frame.first_seen_ms) throw new Error(`Screen ${frame.sequence}: end precedes start`);
    if (frame.grid.length > frame.size.rows || frame.grid.some((row) =>
      row.length > frame.size.cols || row.some((id) => id >= frame.cells.length))) {
      throw new Error(`Screen ${frame.sequence}: invalid cell grid`);
    }
    const width = frame.size.cols * geometry.cell_width;
    const height = frame.size.rows * geometry.cell_height;
    if (!Number.isFinite(width) || !Number.isFinite(height)) throw new Error(`Screen ${frame.sequence}: invalid dimensions`);
    const { cells, grid, svg, ...evidence } = frame;
    return {
      index, sequence: frame.sequence, started: frame.first_seen_ms, ended: frame.last_seen_ms,
      columns: frame.size.cols, rows: frame.size.rows, width, height,
      viewBox: `${geometry.grid_x} ${geometry.grid_y} ${width} ${height}`,
      cursor: { x: frame.cursor.column, y: frame.cursor.row },
      cursorAppearance: `${frame.cursor.shape}, ${frame.cursor.visible ? "visible" : "hidden"}`,
      text: frame.text, svg, omission: frame.omission, cells, grid, evidence,
      label: `#${frame.sequence} / ${time(frame.first_seen_ms)}`,
      description: `${frame.size.cols} x ${frame.size.rows} / ${time(frame.first_seen_ms)} - ${time(frame.last_seen_ms)} / ${frame.changes.join(", ")}`,
    };
  });
  const actionIds = new Set();
  const operations = details.recent_operations ?? [];
  /** @type {TraceAction[]} */
  const actions = operations.map((operation) => {
    if (actionIds.has(operation.sequence)) throw new Error(`Duplicate action sequence: ${operation.sequence}`);
    actionIds.add(operation.sequence);
    if (operation.ended_ms < operation.started_ms) throw new Error(`Action ${operation.sequence}: end precedes start`);
    const display = describeOperation(operation);
    const pointer = operation.input?.mouse_position;
    return {
      id: operation.sequence, screen: operation.screen_at_return, frameIndex: frameIndex.get(operation.screen_at_return),
      ...display, expectation: describeExpectation(operation.expectation),
      unavailableExpectation: operation.expectation?.kind === "unavailable",
      code: !!(display.locator || operation.input),
      searchText: `${display.name} ${display.label} ${operation.safe_summary}`.toLowerCase(),
      failed: operation.result !== "ok", assertion: operation.is_assertion ?? false,
      started: operation.started_ms, ended: operation.ended_ms, duration: operation.ended_ms - operation.started_ms,
      result: operation.result, pointer: pointer && { x: pointer.column, y: pointer.row }, evidence: operation,
    };
  });
  const failureAction = hasFailure ? actions.findLast((action, index) => action.failed &&
    action.screen === timeline.failure_screen_sequence && operations[index].name === details.operation.name) : undefined;
  const mismatches = (details.locator?.stages ?? [])
    .filter((stage) => stage.stage_index === details.locator?.failure_stage)
    .flatMap((stage) => (stage.mismatches ?? []).map((entry) => ({
      ...entry, stage: stage.stage_index, x: entry.location.column,
      y: entry.location.row - (details.locator?.viewport_origin_y ?? 0),
    })));
  const duration = Math.max(0, ...actions.map((action) => action.ended), ...frames.map((frame) => frame.ended));
  const attachmentNames = new Set();
  /** @type {TraceAttachment[]} */
  const attachments = data.attachments.map((file, id) => {
