<script lang="ts">
  import type { Snippet } from "svelte";
  let { id, active, padding = "none", children }: {
    id: string; active: string; padding?: "none" | "metadata" | "details"; children: Snippet;
  } = $props();
</script>

