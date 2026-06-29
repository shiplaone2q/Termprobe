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
      if (!action) return state;
      const next = select(model, state, action.frameIndex, action.id, action.screen);
      return state.detailsTab === "errors" || state.detailsTab === "call"
        ? { ...next, detailsTab: selection(model, next).isFailure ? "errors" : "call" } : next;
    }
    case "jump-to-result":
      return { ...select(model, state, model.failureFrameIndex, model.failureActionId, model.failureScreen), detailsTab: "errors" };
    case "navigate-action": {
      const actions = visibleActions(model, state);
      const position = actions.findIndex((action) => action.id === state.actionId);
      const target = position < 0 ? (event.delta < 0 ? actions.length - 1 : 0) : position + event.delta;
      return actions[target] ? update(model, state, { type: "select-action", id: actions[target].id }) : state;
    }
    case "select-tab":
      return event.tab === "actions" || event.tab === "metadata"
        ? { ...state, sidebarTab: event.tab } : { ...state, detailsTab: event.tab };
    case "filter": return { ...state, filter: event.value, playback: undefined };
    case "assertions-only": return { ...state, assertionsOnly: event.value, playback: undefined };
    case "coordinate": return { ...state, [event.axis]: event.value };
    case "inspect": {
      const frame = selection(model, state).frame;
      const cell = cellAt(frame, event.x, event.y);
      return { ...state, playback: undefined, cell: cell ? { x: event.x, y: event.y } : undefined,
        column: Number.isFinite(event.x) ? String(event.x) : "",
        row: Number.isFinite(event.y) ? String(event.y) : "",
        detailsTab: event.activate ? "cell" : state.detailsTab };
    }
    case "move-cell": {
