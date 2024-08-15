<script lang="ts">
  import type { TraceAction, TraceModel } from "../types.js";
  let { action, isFailure, result }: { action?: TraceAction; isFailure: boolean; result: TraceModel["result"] } = $props();
  const banner = $derived.by(() => {
    const expectation = action?.expectation;
    if (isFailure) return {
      outcome: "failed", summary: expectation ? `Failed: ${expectation}` : `Expected ${result.expected}`, observed: result.actual,
    };
    if (expectation && !action?.unavailableExpectation) return {
      outcome: action?.failed ? "failed" : "passed",
      summary: `${action?.failed ? "Failed" : action?.assertion ? "Passed" : "Completed"}: ${expectation}`,
      observed: action?.failed ? "The selected operation did not complete successfully." : "The selected operation completed successfully.",
    };
    return { outcome: "unknown", summary: expectation || "Expectation not captured for this operation.", observed: "" };
  });
</script>

