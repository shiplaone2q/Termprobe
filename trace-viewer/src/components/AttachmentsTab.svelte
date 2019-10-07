<script lang="ts">
  import type { TraceAttachment } from "../types.js";
  import AttachmentPreview from "./AttachmentPreview.svelte";
  let { files, urls, unavailableFiles, recordingNote, selected, failedImages, onselect, onimageerror }: {
    files: readonly TraceAttachment[]; urls: readonly string[]; unavailableFiles: readonly string[]; recordingNote: string;
    selected?: number; failedImages: readonly number[]; onselect: (id: number) => void; onimageerror: (id: number) => void;
  } = $props();
