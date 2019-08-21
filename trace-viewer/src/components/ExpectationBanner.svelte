<script lang="ts">
  import type { TraceAction, TraceModel } from "../types.js";
  let { action, isFailure, result }: { action?: TraceAction; isFailure: boolean; result: TraceModel["result"] } = $props();
  const banner = $derived.by(() => {
    const expectation = action?.expectation;
    if (isFailure) return {
      outcome: "failed", summary: expectation ? `Failed: ${expectation}` : `Expected ${result.expected}`, observed: result.actual,
    };
    if (expectation && !action?.unavailableExpectation) return {
