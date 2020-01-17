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
