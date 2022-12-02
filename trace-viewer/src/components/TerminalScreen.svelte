<script lang="ts">
  import type { TraceFrame, Geometry, CellMismatch, ViewerState } from "../types.js";
  import CellOverlay from "./CellOverlay.svelte";
  let { frame, url, zoom, geometry, mismatches, cell, oninspect, onmove, onimageerror }: {
    frame?: TraceFrame; url?: string; zoom: ViewerState["zoom"]; geometry: Geometry;
    mismatches: readonly CellMismatch[]; cell?: { x: number; y: number };
    oninspect: (x: number, y: number) => void; onmove: (dx: number, dy: number) => void; onimageerror: (index: number) => void;
  } = $props();
  const width = $derived(!frame ? undefined : zoom === "fit"
    ? `min(${frame.width}px, 100cqw, ${100 * frame.width / frame.height}cqh)`
    : `${frame.width * Number(zoom)}px`);

  function inspect(event: MouseEvent, terminal: HTMLButtonElement) {
    if (!frame) return;
    if (event.detail === 0) {
      oninspect(cell?.x ?? Math.min(frame.cursor.x, frame.columns - 1), cell?.y ?? Math.min(frame.cursor.y, frame.rows - 1));
      return;
    }
    const bounds = terminal.getBoundingClientRect();
    oninspect(Math.floor((event.clientX - bounds.left) * frame.columns / bounds.width),
      Math.floor((event.clientY - bounds.top) * frame.rows / bounds.height));
  }

  function move(event: KeyboardEvent) {
    const moves: Record<string, readonly [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
    const delta = moves[event.key];
    if (delta) { event.preventDefault(); onmove(...delta); }
  }
</script>

