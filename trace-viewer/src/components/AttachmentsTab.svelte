<script lang="ts">
  import type { TraceAttachment } from "../types.js";
  import AttachmentPreview from "./AttachmentPreview.svelte";
  let { files, urls, unavailableFiles, recordingNote, selected, failedImages, onselect, onimageerror }: {
    files: readonly TraceAttachment[]; urls: readonly string[]; unavailableFiles: readonly string[]; recordingNote: string;
    selected?: number; failedImages: readonly number[]; onselect: (id: number) => void; onimageerror: (id: number) => void;
  } = $props();
  const file = $derived(selected === undefined ? undefined : files[selected]);
</script>

<p class="muted">Every available file below is embedded in this HTML. Preview or download it without the original directory. Captured output is untrusted test data; review it before sharing. The embedded JSON describes the evidence at report generation and excludes the HTML's own hash. A separately exported manifest also includes the final HTML hash.</p>
<div class="attachment-layout">
  <div id="evidence-links">
    {#each files as file (file.id)}
