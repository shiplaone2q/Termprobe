<script lang="ts">
  import type { TraceAction } from "../types.js";
  import { time } from "../format.js";
  let { actions, selected, filter, assertionsOnly, onselect, onfilter, onassertions }: {
    actions: readonly TraceAction[]; selected?: number; filter: string; assertionsOnly: boolean;
    onselect: (id: number) => void; onfilter: (filter: string) => void; onassertions: (value: boolean) => void;
  } = $props();
  let points: HTMLElement | undefined = $state();
  $effect(() => {
    if (selected !== undefined) points?.querySelector(`[data-operation="${selected}"]`)?.scrollIntoView({ block: "nearest" });
