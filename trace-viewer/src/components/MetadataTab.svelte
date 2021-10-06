<script lang="ts">
  import type { TraceModel } from "../types.js";
  import PropertyList from "./PropertyList.svelte";
  let { metadata }: { metadata: TraceModel["metadata"] } = $props();
</script>

<h2>Session</h2><PropertyList id="session-properties" values={metadata.properties} />
