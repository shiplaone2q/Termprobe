<script lang="ts">
  import type { TraceAction, TraceFrame, ImageResult } from "../types.js";
  import { time } from "../format.js";
  let { duration, actions, frames, images, selectedAction, selectedFrame, failedImages, onaction, onframe, onimageerror }: {
    duration: number; actions: readonly TraceAction[]; frames: readonly TraceFrame[]; images: readonly ImageResult[];
    selectedAction?: number; selectedFrame?: number; failedImages: readonly number[];
    onaction: (id: number) => void; onframe: (index: number) => void; onimageerror: (index: number) => void;
  } = $props();
</script>

<section class="timeline" aria-label="Trace timeline">
  <div id="time-scale" class="time-scale" aria-hidden="true">
    {#each [0, 1, 2, 3, 4, 5] as tick}<span>{time(Math.round(duration * tick / 5))}</span>{/each}
  </div>
  <div id="action-timeline" class="action-timeline" aria-label="Operation durations">
    {#each actions as action (action.id)}
      {@const title = `${action.name} ${action.label}: ${time(action.duration)} (${action.result})`}
      <button class="action-bar" class:failed={action.failed} data-operation={action.id} {title}
        aria-label={title} aria-current={selectedAction === action.id} onclick={() => onaction(action.id)}
        style:left={`${Math.min(99.5, action.started / Math.max(1, duration) * 100)}%`}
        style:width={`${Math.max(.4, action.duration / Math.max(1, duration) * 100)}%`}></button>
    {/each}
  </div>
  <nav id="filmstrip" class="filmstrip" aria-label="Retained terminal frames">
    {#each frames as frame (frame.index)}
      <button class="thumbnail" data-frame={frame.index} aria-current={selectedFrame === frame.index}
        aria-label={`Screen ${frame.sequence} at ${time(frame.started)}`} onclick={() => onframe(frame.index)}>
