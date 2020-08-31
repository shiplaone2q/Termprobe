<script lang="ts">
  import type { Snippet } from "svelte";
  import type { SidebarTab } from "../types.js";
  import TabBar from "./TabBar.svelte";
  import TabPanel from "./TabPanel.svelte";
  let { active, onselect, actionsPanel, metadataPanel }: {
    active: SidebarTab; onselect: (tab: SidebarTab) => void; actionsPanel: Snippet; metadataPanel: Snippet;
  } = $props();
  const tabs: readonly { id: SidebarTab; label: string }[] = [{ id: "actions", label: "Actions" }, { id: "metadata", label: "Metadata" }];
</script>

