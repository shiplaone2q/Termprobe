<script lang="ts">
  let { id, locator }: { id: string; locator?: string } = $props();
</script>

<div id={`${id}-section`} hidden={!locator}>
  <h2>Locator</h2><pre {id} class="locator-expression">{locator ?? ""}</pre>
</div>

<style>
