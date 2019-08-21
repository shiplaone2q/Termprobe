<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import type { TraceModel, Resources, ViewerEvent, ViewerState } from "./types.js";
  import { initialState, update, PLAYBACK_INTERVAL } from "./state.js";
  import { selection, visibleActions, inspectCell } from "./model.js";
  import Header from "./components/Header.svelte";
  import Timeline from "./components/Timeline.svelte";
  import SideNav from "./components/SideNav.svelte";
  import BottomNav from "./components/BottomNav.svelte";
  import TerminalViewer from "./components/TerminalViewer.svelte";
  import ActionsTab from "./components/ActionsTab.svelte";
  import MetadataTab from "./components/MetadataTab.svelte";
  import ResultTab from "./components/ResultTab.svelte";
  import CallTab from "./components/CallTab.svelte";
  import CellTab from "./components/CellTab.svelte";
  import AttachmentsTab from "./components/AttachmentsTab.svelte";

  let { model, resources, hash = "" }: { model: TraceModel; resources: Resources; hash?: string } = $props();
  let view = $state.raw<ViewerState>(untrack(() => initialState(model, hash)));
  const selected = $derived(selection(model, view));
  const actions = $derived(visibleActions(model, view));
  const inspection = $derived(inspectCell(model, view));
  const playback = $derived(view.playback);

  function send(event: ViewerEvent) {
    view = update(model, view, event);
  }

  $effect(() => {
    if (!playback) return;
    const timer = window.setInterval(() => send({ type: "tick" }), PLAYBACK_INTERVAL);
    return () => window.clearInterval(timer);
  });

