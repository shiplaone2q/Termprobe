<script lang="ts">
  import type { Properties } from "../types.js";
  let { summary, hasFailure, canJump, onjump }: {
    summary: Properties; hasFailure: boolean; canJump: boolean; onjump: () => void;
  } = $props();
</script>

<header class="topbar">
  <strong class="brand">tui-test <span>Trace viewer</span></strong>
  <div id="session-summary" class="session-summary">
    {#each Object.entries(summary) as [label, value] (label)}
      <span class="metadata-item" title={`${label}: ${value}`}>
        <span class="metadata-label">{label}:</span><span class="metadata-value">{value}</span>
