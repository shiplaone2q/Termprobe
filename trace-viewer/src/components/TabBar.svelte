<script lang="ts" generics="T extends string">
  let { tabs, active, label, group, onselect }: {
    tabs: readonly { id: T; label: string }[]; active: T; label: string; group: string; onselect: (tab: T) => void;
  } = $props();
  let buttons: HTMLButtonElement[] = [];
