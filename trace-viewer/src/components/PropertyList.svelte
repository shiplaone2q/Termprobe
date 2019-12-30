<script lang="ts">
  import type { Properties } from "../types.js";
  let { id, values, colors = {} }: { id: string; values: Properties; colors?: Readonly<Record<string, string>> } = $props();
</script>
