<script lang="ts">
  import type { TraceFrame, TraceAction, TraceModel, ImageResult, Geometry, CellMismatch, ViewerState } from "../types.js";
  import { IMAGE_ERROR } from "../resources.js";
  import ExpectationBanner from "./ExpectationBanner.svelte";
  import TerminalScreen from "./TerminalScreen.svelte";
  let { frame, action, missingScreen, sharesFailure, isFailure, result, actions, frameCount, zoom, playing,
    image, imageFailed, geometry, mismatches, cell, onaction, onframe, onplay, onzoom, oninspect, onmove, onimageerror }: {
    frame?: TraceFrame; action?: TraceAction; missingScreen?: number; sharesFailure: boolean; isFailure: boolean;
    result: TraceModel["result"]; actions: readonly TraceAction[]; frameCount: number; zoom: ViewerState["zoom"]; playing: boolean;
    image?: ImageResult; imageFailed: boolean; geometry: Geometry; mismatches: readonly CellMismatch[]; cell?: { x: number; y: number };
    onaction: (delta: -1 | 1) => void; onframe: (index: number) => void; onplay: () => void;
    onzoom: (zoom: ViewerState["zoom"]) => void; oninspect: (x: number, y: number) => void;
    onmove: (dx: number, dy: number) => void; onimageerror: (index: number) => void;
  } = $props();
  const index = $derived(frame?.index ?? -1);
  const position = $derived(actions.findIndex((item) => item.id === action?.id));
  const imageError = $derived(imageFailed ? IMAGE_ERROR : image?.error);
  const notice = $derived([frame?.omission, imageError].filter(Boolean).join(" "));
  const meta = $derived(frame
    ? `Screen ${frame.sequence}${sharesFailure ? (isFailure ? " / failure" : " / also used by failure") : ""} / ${frame.description}`
    : `Screen ${missingScreen ?? "unknown"} was not retained. No substitute frame is shown.`);

  function changeZoom(event: Event & { currentTarget: HTMLSelectElement }) {
    const value = event.currentTarget.value;
    if (value !== "fit" && value !== "1" && value !== "1.5") throw new Error(`Unknown zoom: ${value}`);
    onzoom(value);
  }
</script>

<section class="viewer" aria-label="Terminal snapshot">
  <div class="viewer-toolbar">
    <strong id="frame-heading">{action?.name ?? "Terminal snapshot"}</strong>
    <div class="toolbar">
      <button id="previous-point" aria-label="Previous action" disabled={!actions.length || position === 0} onclick={() => onaction(-1)}>Prev action</button>
      <button id="next-point" aria-label="Next action" disabled={!actions.length || position === actions.length - 1} onclick={() => onaction(1)}>Next action</button>
      <label class="zoom-label">Zoom <select id="zoom" aria-label="Terminal zoom" value={zoom} onchange={changeZoom}>
        <option value="fit">Fit</option><option value="1">100%</option><option value="1.5">150%</option>
      </select></label>
    </div>
  </div>
  <ExpectationBanner {action} {isFailure} {result} />
