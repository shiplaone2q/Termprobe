<script lang="ts">
  import type { TraceFrame, Geometry, CellMismatch } from "../types.js";
  import { cellAt } from "../model.js";
  let { frame, geometry, mismatches, cell }: {
    frame: TraceFrame; geometry: Geometry; mismatches: readonly CellMismatch[]; cell?: { x: number; y: number };
  } = $props();
  const locations = $derived([...new Set(mismatches.filter((entry) => cellAt(frame, entry.x, entry.y)).map((entry) => `${entry.x},${entry.y}`))]);
</script>

<svg id="overlay" aria-hidden="true" viewBox={frame.viewBox}>
  {#each locations as location (location)}
    {@const [x, y] = location.split(",").map(Number)}
