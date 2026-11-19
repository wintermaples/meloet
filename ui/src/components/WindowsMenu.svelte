<!-- 状態行の「窓」の一覧（仕様書 2 章「状態行の『窓』」、モック F-WindowOpen）。幅 380、行は高さ 24 で、窓の印、プラグインの名前、居場所。
     いちばん下に「ぜんぶ前に出す」。行を押すとその窓を前に出す（OpenPluginWindow）。ひらいたら最初の行にフォーカスを置き、↑ ↓ でえらび、Enter で決める。
     Esc と外を押したときは StatusLine がとじる（メニューと同じ番）。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { Command, PluginWindowView } from "../bridge";
  import { t } from "../text/ja";
  import WindowMark from "./WindowMark.svelte";

  interface Props {
    list: readonly PluginWindowView[];
    /** 居場所の文（「1 ピアノ ・ 音源」「A リバーブ ・ インサート 1」「棚 ・ ためし弾き」）。 */
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
    border-bottom: var(--line-hair) solid var(--color-gray-4);
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
