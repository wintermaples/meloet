<!-- The scale list (spec 2 "List": width 220, background gray.3, border gray.8, rows 24 high with a choice mark and a name, heading rows 「hd」).
     The content is the owner's decision: under the heading 「種類」, 「メジャー」 and 「マイナー」; under the heading 「主音」, C to B.
     On open, focus goes to the row of the chosen kind. ↑ ↓ choose, Enter decides. On Esc or pressing outside, the side that opened it closes it. -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { MusicalScale, ScaleMode } from "../../bridge";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { t } from "../../text/ja";
  import { pitchClassName } from "../format";

  interface Props {
    scale: MusicalScale;
    /** Chosen. Only the kind, or only the root, changes. */
    onpick: (root: number, mode: ScaleMode) => void;
  }
  let { scale, onpick }: Props = $props();

  interface Row {
    label: string;
    selected: boolean;
    root: number;
    mode: ScaleMode;
  }
  const modes = $derived<Row[]>([
    { label: t("scale.major"), selected: scale.mode === "major", root: scale.root, mode: "major" },
    { label: t("scale.minor"), selected: scale.mode === "minor", root: scale.root, mode: "minor" },
  ]);
  const roots = $derived<Row[]>(
    Array.from({ length: 12 }, (_, root) => ({ label: pitchClassName(root), selected: root === scale.root, root, mode: scale.mode })),
  );

  let list = $state<HTMLElement>();
  const buttons = () => [...(list?.querySelectorAll<HTMLButtonElement>("button") ?? [])];

  onMount(() =>
    queueMicrotask(() => {
      const all = buttons();
      (all.find((b) => b.getAttribute("aria-checked") === "true") ?? all[0] ?? list)?.focus();
    }),
  );

  function onkeydown(ev: KeyboardEvent): void {
    if (ev.altKey || ev.ctrlKey || ev.metaKey || ev.shiftKey || ev.isComposing) return;
    const all = buttons();
    const i = all.indexOf(document.activeElement as HTMLButtonElement);
    if (ev.key === "ArrowUp" || ev.key === "ArrowDown") {
      const next = ev.key === "ArrowUp" ? Math.max(0, i - 1) : Math.min(all.length - 1, i + 1);
      all[next]?.focus();
    } else if (ev.key === "Enter") {
      all[i]?.click();
    } else {
      return;
    }
    ev.preventDefault();
    ev.stopPropagation();
  }
</script>

{#snippet group(heading: string, rows: Row[])}
  <div class="head"><span class="m-lbl">{heading}</span></div>
  {#each rows as row (row.label)}
    <button class="m-plain row m-focus-in" role="menuitemradio" aria-checked={row.selected} onclick={() => onpick(row.root, row.mode)}>
      <span class="choice" class:selected={row.selected}><span class="dot"></span></span>
      <span class="label m-num">{row.label}</span>
    </button>
  {/each}
{/snippet}

<div class="menu" role="menu" tabindex="-1" aria-label={t("inspector.musicalScale")} bind:this={list} {onkeydown} data-test="scale-menu">
  <ScrollArea>
    {@render group(t("scale.kind"), modes)}
    {@render group(t("scale.root"), roots)}
  </ScrollArea>
</div>

<style>
  .menu {
    width: var(--part-list-width);
    max-height: 100%;
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
  }
  .row {
    width: 100%;
    height: var(--part-list-row);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-gray-4);
    white-space: nowrap;
  }
  /* Hover (spec 2: border gray.8, text gray.10), pressed (bg gray.2, border gray.8). The border is drawn inside the row,
     so it does not mix with the 「選んだ行」 (background gray.4, a line.mark accent on the left) (docs/history/design-notes.ja.md A-6). */
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
  /* The text is gray.10, the same as the 「拡大率」 list in the status line (ChoiceMenu, mock F-Zoom). */
  .label {
    color: var(--color-gray-10);
  }
</style>
