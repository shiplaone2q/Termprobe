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
  });
</script>

<div class="action-filter">
  <input id="action-filter" type="search" aria-label="Filter actions" placeholder="Filter actions"
    value={filter} oninput={(event) => onfilter(event.currentTarget.value)} />
  <label><input id="assertions-only" type="checkbox" checked={assertionsOnly}
    onchange={(event) => onassertions(event.currentTarget.checked)} /> Assertions and waits only</label>
</div>
<nav id="points" bind:this={points} aria-label="Recorded actions">
  {#each actions as action (action.id)}
    <button class="point" class:failed={action.failed} data-operation={action.id} aria-current={selected === action.id}
      title={`${action.name} ${action.label} / screen ${action.screen}`} aria-label={`${action.name} ${action.label} ${action.result}`}
      onclick={() => onselect(action.id)}>
      <span class="status"></span>
      <span class="action-label">
        <span class="action-line"><span class="name">{action.name}</span>{#if action.code}<code class="query-code">{action.label}</code>{/if}</span>
        {#if !action.code}<span class="description">{action.label}</span>{/if}
