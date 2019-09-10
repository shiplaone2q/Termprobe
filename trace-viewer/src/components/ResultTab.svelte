<script lang="ts">
  import type { TraceModel } from "../types.js";
  import LocatorBlock from "./LocatorBlock.svelte";
  let { result, hasFailure }: { result: TraceModel["result"]; hasFailure: boolean } = $props();
</script>

<div class="error-heading">
  <h1 id="operation" class:passed={!hasFailure}>{result.title}</h1><span id="failure-timing" class="muted">{result.timing}</span>
</div>
