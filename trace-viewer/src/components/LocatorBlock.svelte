<script lang="ts">
  let { id, locator }: { id: string; locator?: string } = $props();
</script>

<div id={`${id}-section`} hidden={!locator}>
