<!-- The 「拡大率」 list of the status line (spec 2 "「拡大率」 in the status line", mock F-Zoom). On open, focus goes to the row of the current scale.
     ↑ ↓ choose, Enter decides. On Esc or a press outside, StatusLine closes it. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { SCALES, type Scale } from "../design/tokens";
  import type { Platform } from "../keys/keys";
  import { t } from "../text/ja";

  interface Props {
    scale: number;
    /** Scales that do not fit the current screen. */
    unfit: readonly number[];
    platform: Platform;
    onpick: (scale: Scale) => void;
  }
  let { scale, unfit, platform, onpick }: Props = $props();

  let list = $state<HTMLElement>();

  // docs/history/design-notes.ja.md A-2: 「Ctrl Alt 1」 on Linux and Windows, 「Cmd Option 1」 on Mac.
  const keyLabel = (n: number) =>
    platform === "mac" ? `${t("key.cmd")} ${t("key.option")} ${n}` : `${t("key.ctrl")} ${t("key.alt")} ${n}`;

  const usableRows = () => [...(list?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];

  // Set after StatusLine moves the list to body (moving it loses focus).
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
  /* Hover (spec 2: border gray.8, text gray.10), pressed (bg gray.2, border gray.8). The border is drawn inside the row
     so it does not mix with 「選んだ行」 (bg gray.4, left line.mark in accent) (docs/history/design-notes.ja.md A-6). */
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

  /* Disabled (spec 2): border gray.5, text gray.7, cannot be pressed. The key is gray.7 too. The reason text stays gray.8 (mock F-Zoom). */
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
