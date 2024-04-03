import { cellAt, selection, visibleActions } from "./model.js";

/** @import { TraceModel, ViewerState, ViewerEvent } from "./types.js" */
export const PLAYBACK_INTERVAL = 400;

/** @param {TraceModel} model @param {string} hash @returns {ViewerState} */
export function initialState(model, hash = "") {
  /** @type {ViewerState} */
  const state = {
    sidebarTab: "actions", detailsTab: "errors", filter: "", assertionsOnly: false,
    column: "0", row: "0", zoom: "fit", failedImages: [], failedAttachments: [],
  };
  const link = /^#screen-(\d+)$/.exec(hash);
  const index = link ? model.frames.findIndex((frame) => frame.sequence === Number(link[1])) : -1;
  return index >= 0 ? select(model, state, index) : select(model, state, model.failureFrameIndex, model.failureActionId, model.failureScreen);
}

/** @param {TraceModel} model @param {ViewerState} state @param {number | undefined} frameIndex
 * @param {number} [actionId] @param {number} [missingScreen] @returns {ViewerState}
 */
function select(model, state, frameIndex, actionId, missingScreen) {
  const next = { ...state, frameIndex, actionId, missingScreen, playback: undefined };
  const { frame, action, mismatches } = selection(model, next);
  const mismatch = mismatches.find((entry) => cellAt(frame, entry.x, entry.y));
  const point = mismatch ?? action?.pointer ?? (frame && {
    x: Math.min(frame.cursor.x, frame.columns - 1), y: Math.min(frame.cursor.y, frame.rows - 1),
  });
  return { ...next, column: String(point?.x ?? 0), row: String(point?.y ?? 0),
    cell: point && cellAt(frame, point.x, point.y) ? { x: point.x, y: point.y } : undefined };
}

/** @param {TraceModel} model @param {ViewerState} state @param {ViewerEvent} event @returns {ViewerState} */
export function update(model, state, event) {
  switch (event.type) {
    case "select-frame":
      return Number.isInteger(event.index) && model.frames[event.index] ? select(model, state, event.index) : state;
    case "select-action": {
      const action = model.actions.find((action) => action.id === event.id);
