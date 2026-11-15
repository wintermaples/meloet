<!-- 見出しと行の一覧（状態行の「拡大率」の一覧と同じ部品。仕様書 2 章、モック F-Zoom）。
     selected を渡すと選ぶ印つきの一覧、渡さなければすることの一覧（印なし、右はしにキー）。
     ひらいたら、えらんでいる行（なければ使える最初の行）にフォーカスを置く。↑ ↓ でえらび、Enter で決める。Esc と外を押したときは、ひらいた側がとじる。 -->
<script lang="ts" generics="V">
  import { onMount } from "svelte";

  interface Row {
    value: V;
    label: string;
    /** 右はしのキーの札。 */
    key?: string;
    /** false なら「使えない」（仕様書 2 章）。 */
    usable?: boolean;
  }
  interface Props {
    /** null なら見出しの行を出さない。 */
    heading: string | null;
    /** 見出しがないときの名前（読み上げ用）。 */
    label?: string;
    rows: readonly Row[];
    selected?: V;
    onpick: (value: V) => void;
    test: string;
  }
  let { heading, label, rows, selected, onpick, test }: Props = $props();
  const marks = $derived(selected !== undefined);

  let list = $state<HTMLElement>();
  const buttons = () => [...(list?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];

  // ひらいた側が一覧を body へ移したあとで置く（移すとフォーカスが外れるので）。
  onMount(() =>
    queueMicrotask(() => {
      const all = buttons();
      const at = all.find((b) => b.getAttribute("aria-checked") === "true") ?? all[0];
      (at ?? list)?.focus();
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
      if (i >= 0) onpick(rows[Number(all[i]!.dataset.row)]!.value);
    } else {
      return;
    }
    ev.preventDefault();
    ev.stopPropagation();
  }
</script>

<div class="menu" role="menu" tabindex="-1" aria-label={heading ?? label} bind:this={list} {onkeydown} data-test={test}>
  {#if heading !== null}<div class="head"><span class="m-lbl">{heading}</span></div>{/if}
  {#each rows as row, i (i)}
    {@const usable = row.usable ?? true}
    <button
      class="m-plain row"
      class:unusable={!usable}
      role={marks ? "menuitemradio" : "menuitem"}
      aria-checked={marks ? row.value === selected : undefined}
      disabled={!usable}
      data-row={i}
      onclick={() => onpick(row.value)}
    >
      {#if marks}<span class="choice" class:selected={row.value === selected}><span class="dot"></span></span>{/if}
      <span class="label m-num">{row.label}</span>
      {#if row.key}<span class="m-spacer"></span><span class="m-num m-lbl">{row.key}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .menu {
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
  .label {
    color: var(--color-gray-10);
  }
  .row:disabled {
    cursor: default;
  }
  /* 使えない（仕様書 2 章）: 文字 gray.7 ・ 押せない。キーは gray.8 のまま（モック F-Zoom）。 */
  .unusable .label {
    color: var(--color-gray-7);
  }
</style>
