<script lang="ts">
  import type { Properties } from "../types.js";
  let { id, values, colors = {} }: { id: string; values: Properties; colors?: Readonly<Record<string, string>> } = $props();
</script>

<dl {id}>
  {#each Object.entries(values) as [key, value] (key)}
    <dt>{key}</dt>
    <dd>{#if /^#[0-9a-f]{6}$/i.test(colors[key] ?? "")}<span class="swatch" style:background-color={colors[key]}></span>{/if}{value ?? "Not captured"}</dd>
  {/each}
</dl>

