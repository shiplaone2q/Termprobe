<script lang="ts" generics="T extends string">
  let { tabs, active, label, group, onselect }: {
    tabs: readonly { id: T; label: string }[]; active: T; label: string; group: string; onselect: (tab: T) => void;
  } = $props();
  let buttons: HTMLButtonElement[] = [];

  function navigate(event: KeyboardEvent, index: number) {
    const offsets: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, Home: -index, End: tabs.length - 1 - index };
