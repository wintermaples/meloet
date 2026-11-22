<!-- 状態行の「拡大率」の一覧（仕様書 2 章「状態行の『拡大率』」、モック F-Zoom）。ひらいたら、いまの率の行にフォーカスを置く。
     ↑ ↓ でえらび、Enter で決める。Esc と外を押したときは StatusLine がとじる。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import { SCALES, type Scale } from "../design/tokens";
  import type { Platform } from "../keys/keys";
  import { t } from "../text/ja";

  interface Props {
    scale: number;
    /** いまの画面に入らない率。 */
    unfit: readonly number[];
    platform: Platform;
    onpick: (scale: Scale) => void;
  }
  let { scale, unfit, platform, onpick }: Props = $props();

  let list = $state<HTMLElement>();

  // docs/history/design-notes.ja.md A-2: Linux と Windows では「Ctrl Alt 1」、Mac では「Cmd Option 1」。
  const keyLabel = (n: number) =>
    platform === "mac" ? `${t("key.cmd")} ${t("key.option")} ${n}` : `${t("key.ctrl")} ${t("key.alt")} ${n}`;

  const usableRows = () => [...(list?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];

  // StatusLine が一覧を body へ移したあとで置く（移すとフォーカスが外れるので）。
  onMount(() =>
    queueMicrotask(() => {
      const rows = usableRows();
      (rows.find((b) => b.dataset.scale === String(scale)) ?? rows[0])?.focus();
    }),
  );

  function onkeydown(ev: KeyboardEvent): void {
    if (ev.altKey || ev.ctrlKey || ev.metaKey || ev.shiftKey || ev.isComposing) return;
    const rows = usableRows();
    const i = rows.indexOf(document.activeElement as HTMLButtonElement);
    if (ev.key === "ArrowUp" || ev.key === "ArrowDown") {
      const next = ev.key === "ArrowUp" ? Math.max(0, i - 1) : Math.min(rows.length - 1, i + 1);
      rows[next]?.focus();
    } else if (ev.key === "Enter") {
      if (i >= 0) onpick(Number(rows[i]!.dataset.scale) as Scale);
    } else {
      return;
    }
    ev.preventDefault();
    ev.stopPropagation();
  }
</script>

<div class="zoom-menu" role="menu" tabindex="-1" aria-label={t("zoom.heading")} bind:this={list} {onkeydown} data-test="zoom-menu">
  <div class="head"><span class="m-lbl">{t("zoom.heading")}</span></div>
  {#each SCALES as s, i (s)}
    {@const usable = !unfit.includes(s)}
    <button
      class="m-plain row m-focus-in"
      class:unusable={!usable}
      role="menuitemradio"
      aria-checked={s === scale}
      disabled={!usable}
      data-scale={s}
      onclick={() => onpick(s)}
    >
      <span class="choice" class:selected={s === scale}><span class="dot"></span></span>
      <span class="rate m-num">{t("status.scale", { scale: s })}</span>
      {#if !usable}<span class="m-lbl" data-test="zoom-unfit">{t("msg.scaleDoesNotFit")}</span>{/if}
      <span class="m-spacer"></span>
      <span class="m-num m-lbl key" data-test="zoom-key">{keyLabel(i + 1)}</span>
    </button>
  {/each}
</div>

<style>
  .zoom-menu {
    width: var(--part-zoomlist-width);
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
    gap: var(--space-2);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    flex-shrink: 0;
  }
  .row {
    height: var(--part-zoomlist-row);
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
  /* 乗った（仕様書 2 章: 枠 gray.8 ・ 文字 gray.10）・押した（bg gray.2 ・ 枠 gray.8）。枠は行の内がわに描き、
     「選んだ行」（地 gray.4 ・ 左 line.mark の accent）とまざらないようにする（docs/history/design-notes.ja.md A-6）。 */
  .row:hover:not(:disabled) {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .row:hover:not(:disabled) :global(.m-lbl) {
    color: var(--color-gray-10);
  }
  .row:active:not(:disabled) {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .row:disabled {
    cursor: default;
  }
  .choice {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--part-choice-size);
    height: var(--part-choice-size);
    border: var(--line-hair) solid var(--color-gray-8);
  }
  .choice.selected {
    border-color: var(--color-accent);
  }
  .dot {
    width: var(--part-choice-dot);
    height: var(--part-choice-dot);
  }
  .choice.selected .dot {
    background: var(--color-accent);
  }
  .rate {
    flex-shrink: 0;
    width: var(--mock-zoom-rate);
    color: var(--color-gray-10);
  }

  /* 使えない（仕様書 2 章）: 枠 gray.5 ・ 文字 gray.7 ・ 押せない。キーも gray.7。理由の文は gray.8 のまま（モック F-Zoom）。 */
  .unusable .choice {
    border-color: var(--color-gray-5);
  }
  .unusable .choice .dot {
    background: none;
  }
  .unusable .rate,
  .unusable .key {
    color: var(--color-gray-7);
  }
</style>
