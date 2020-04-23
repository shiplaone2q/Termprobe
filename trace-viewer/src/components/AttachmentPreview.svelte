<script lang="ts">
  import type { TraceAttachment } from "../types.js";
  let { file, url, failed, onimageerror }: { file?: TraceAttachment; url?: string; failed: boolean; onimageerror: (id: number) => void } = $props();
</script>

<div id="attachment-preview">
  {#if !file}
    <p class="muted">Select an attachment to preview it.</p>
  {:else}
    <h2>{file.name}</h2>
