<script lang="ts">
  import type { TraceAttachment } from "../types.js";
  let { file, url, failed, onimageerror }: { file?: TraceAttachment; url?: string; failed: boolean; onimageerror: (id: number) => void } = $props();
</script>

<div id="attachment-preview">
  {#if !file}
