<script lang="ts">
  import type { TraceModel } from "../types.js";
  import PropertyList from "./PropertyList.svelte";
  let { metadata }: { metadata: TraceModel["metadata"] } = $props();
</script>

