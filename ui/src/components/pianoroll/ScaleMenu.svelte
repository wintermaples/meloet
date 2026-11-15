<!-- 音階の一覧（仕様書 2 章「一覧」: 幅 220、地 gray.3、枠 gray.8、行は高さ 24 で選ぶ印と名前、見出しの行は「hd」）。
     中身はヤチヨさんの決め（2026-11-15）: 見出し「種類」の下に メジャー ・ マイナー、見出し「主音」の下に C 〜 B。
     ひらいたら、えらんでいる種類の行にフォーカスを置く。↑ ↓ でえらび、Enter で決める。Esc と外を押したときは、ひらいた側がとじる。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { MusicalScale, ScaleMode } from "../../bridge";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { t } from "../../text/ja";
  import { pitchClassName } from "../format";

  interface Props {
    scale: MusicalScale;
    /** えらんだ。種類だけ、または主音だけが変わる。 */
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
    <button class="m-plain row" role="menuitemradio" aria-checked={row.selected} onclick={() => onpick(row.root, row.mode)}>
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
  /* 文字は状態行の「拡大率」の一覧（ChoiceMenu、モック F-Zoom）と同じ gray.10。 */
  .label {
    color: var(--color-gray-10);
  }
</style>
