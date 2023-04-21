<script lang="ts">
  import type { Snippet } from "svelte";
  let { id, active, padding = "none", children }: {
    id: string; active: string; padding?: "none" | "metadata" | "details"; children: Snippet;
  } = $props();
</script>

<div class="tab-panel" id={`${id}-panel`} role="tabpanel" tabindex="0" aria-labelledby={`${id}-tab`} hidden={active !== id}
  class:metadata={padding === "metadata"} class:details={padding === "details"}>
  {@render children()}
