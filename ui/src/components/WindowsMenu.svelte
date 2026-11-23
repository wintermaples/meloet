<!-- The 「窓」 list of the status line (spec 2 "「窓」 in the status line", mock F-WindowOpen). Width 380, rows 24 high: window mark, plugin name, where it lives.
     「ぜんぶ前に出す」 at the bottom. Pressing a row brings that window to the front (OpenPluginWindow). On open, focus goes to the first row; ↑ ↓ choose, Enter decides.
     On Esc or a press outside, StatusLine closes it (the same turn as menus). -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { Command, PluginWindowView } from "../bridge";
  import { t } from "../text";
  import WindowMark from "./WindowMark.svelte";

  interface Props {
    list: readonly PluginWindowView[];
    /** Where it lives (「1 ピアノ ・ 音源」「A リバーブ ・ インサート 1」「棚 ・ ためし弾き」). */
    placeOf: (w: PluginWindowView) => string;
    onpick: (command: Command) => void;
  }
  let { list, placeOf, onpick }: Props = $props();

  let menu = $state<HTMLElement>();
  const rows = () => [...(menu?.querySelectorAll<HTMLButtonElement>("button") ?? [])];

  onMount(() => queueMicrotask(() => rows()[0]?.focus()));

  function onkeydown(ev: KeyboardEvent): void {
    if (ev.altKey || ev.ctrlKey || ev.metaKey || ev.shiftKey || ev.isComposing) return;
    const all = rows();
    const i = all.indexOf(document.activeElement as HTMLButtonElement);
    if (ev.key === "ArrowUp" || ev.key === "ArrowDown") {
      all[ev.key === "ArrowUp" ? Math.max(0, i - 1) : Math.min(all.length - 1, i + 1)]?.focus();
    } else if (ev.key === "Enter") {
      all[i]?.click();
    } else return;
    ev.preventDefault();
    ev.stopPropagation();
  }
</script>

<div class="windows-menu" role="menu" tabindex="-1" aria-label={t("windows.heading")} bind:this={menu} {onkeydown} data-test="windows-menu">
  <div class="head"><span class="m-lbl">{t("windows.heading")}</span></div>
  {#each list as w, i (i)}
    <button class="m-plain row m-focus-in" role="menuitem" data-test="windows-row" onclick={() => onpick({ type: "openPluginWindow", slot: w.slot })}>
      <WindowMark />
      <span class="name">{w.name}</span>
      <span class="m-spacer"></span>
      <span class="m-lbl place">{placeOf(w)}</span>
    </button>
  {/each}
  <button class="m-plain row m-focus-in" role="menuitem" data-test="windows-raise-all" onclick={() => onpick({ type: "raisePluginWindows" })}>
    <span class="name">{t("windows.raiseAll")}</span>
  </button>
</div>

<style>
  .windows-menu {
    width: var(--part-windowlist-width);
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-8);
    outline: none;
  }
  .head {
    height: var(--size-row);
    display: flex;
    align-items: center;
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    flex-shrink: 0;
  }
  .row {
    height: var(--part-windowlist-row);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-line-row);
    white-space: nowrap;
    flex-shrink: 0;
  }
  .row:last-child {
    border-bottom: 0;
  }
  .row:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .row:hover :global(.m-lbl) {
    color: var(--color-gray-10);
  }
  .row:active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-gray-10);
  }
  .place {
    flex-shrink: 0;
  }
</style>
