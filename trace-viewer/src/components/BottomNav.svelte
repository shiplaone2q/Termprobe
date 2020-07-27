<script lang="ts">
  import type { Snippet } from "svelte";
  import type { DetailsTab } from "../types.js";
  import TabBar from "./TabBar.svelte";
  import TabPanel from "./TabPanel.svelte";
  let { active, hasFailure, onselect, resultPanel, cellPanel, callPanel, attachmentsPanel }: {
    active: DetailsTab; hasFailure: boolean; onselect: (tab: DetailsTab) => void;
    resultPanel: Snippet; cellPanel: Snippet; callPanel: Snippet; attachmentsPanel: Snippet;
