<script lang="ts">
  import type { TraceAction, TraceFrame } from "../types.js";
  import { json, time } from "../format.js";
  import PropertyList from "./PropertyList.svelte";
  import LocatorBlock from "./LocatorBlock.svelte";
  let { action, frame, missingScreen }: { action?: TraceAction; frame?: TraceFrame; missingScreen?: number } = $props();
  const properties = $derived({
