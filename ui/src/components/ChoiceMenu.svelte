<!-- 一覧（仕様書 2 章「一覧」、モック F-Menus）。幅 220、行は高さ 24 で、選ぶ印、名前、キー。見出しの行もまぜられる。
     えらんでいる行は、selected と同じ値の行か、checked の行。することの一覧（曲の名前の一覧）は、印をからのまま出す。
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
    /** えらんでいるか（selected を使わない行）。 */
    checked?: boolean;
  }
  /** 一覧の中の見出しの行。 */
  interface Heading {
    heading: string;
  }
  interface Props {
    /** null なら見出しの行を出さない。 */
    heading: string | null;
    /** 見出しがないときの名前（読み上げ用）。 */
    label?: string;
    rows: readonly (Row | Heading)[];
    selected?: V;
    onpick: (value: V) => void;
    test: string;
  }
  let { heading, label, rows, selected, onpick, test }: Props = $props();
  const isHeading = (r: Row | Heading): r is Heading => "heading" in r;
  /** undefined なら、えらぶ行ではない（することの行）。 */
  const checkedOf = (r: Row): boolean | undefined => r.checked ?? (selected !== undefined ? r.value === selected : undefined);

  let list = $state<HTMLElement>();
  const buttons = () => [...(list?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];

  // 窓からはみ出すときは、窓の中に入るまで左（上）へずらす。ひらいた側が置いた所からのずれ。
  let shift = $state({ x: 0, y: 0 });
  function keepInside(): void {
    if (!list) return;
    const r = list.getBoundingClientRect();
    const left = r.left - shift.x;
    const top = r.top - shift.y;
    const x = Math.max(-left, Math.min(0, window.innerWidth - (left + r.width)));
    const y = Math.max(-top, Math.min(0, window.innerHeight - (top + r.height)));
    if (x !== shift.x || y !== shift.y) shift = { x, y };
  }

  // ひらいた側が一覧を body へ移して置いたあとで、ずらしてからフォーカスを置く（移すとフォーカスが外れるので）。
  onMount(() => {
    queueMicrotask(() => {
      keepInside();
      const all = buttons();
      const at = all.find((b) => b.getAttribute("aria-checked") === "true") ?? all[0];
      (at ?? list)?.focus();
    });
    const onResize = () => requestAnimationFrame(keepInside);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });

  function onkeydown(ev: KeyboardEvent): void {
    if (ev.altKey || ev.ctrlKey || ev.metaKey || ev.shiftKey || ev.isComposing) return;
    const all = buttons();
    const i = all.indexOf(document.activeElement as HTMLButtonElement);
    if (ev.key === "ArrowUp" || ev.key === "ArrowDown") {
      const next = ev.key === "ArrowUp" ? Math.max(0, i - 1) : Math.min(all.length - 1, i + 1);
      all[next]?.focus();
    } else if (ev.key === "Enter") {
      if (i >= 0) onpick((rows[Number(all[i]!.dataset.row)] as Row).value);
    } else {
      return;
    }
    ev.preventDefault();
    ev.stopPropagation();
  }
</script>

<div
  class="menu"
  role="menu"
  tabindex="-1"
  aria-label={heading ?? label}
  style:transform={shift.x || shift.y ? `translate(${shift.x}px, ${shift.y}px)` : null}
  bind:this={list}
  {onkeydown}
  data-test={test}
>
  {#if heading !== null}<div class="head"><span class="m-lbl">{heading}</span></div>{/if}
  {#each rows as row, i (i)}
    {#if isHeading(row)}
      <div class="head" role="presentation"><span class="m-lbl">{row.heading}</span></div>
    {:else}
      {@const usable = row.usable ?? true}
      {@const checked = checkedOf(row)}
      <button
        class="m-plain row m-focus-in"
        class:unusable={!usable}
        role={checked === undefined ? "menuitem" : row.checked !== undefined ? "menuitemcheckbox" : "menuitemradio"}
        aria-checked={checked}
        disabled={!usable}
        data-row={i}
        onclick={() => onpick(row.value)}
      >
        <span class="choice" class:selected={checked === true}><span class="dot"></span></span>
        <span class="label m-num">{row.label}</span>
        {#if row.key}<span class="m-spacer"></span><span class="m-num m-lbl">{row.key}</span>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .menu {
    width: var(--part-list-width);
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
    height: var(--part-list-row);
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
     「選んだ行」（地 gray.4 ・ 左 line.mark の accent）とまざらないようにする（docs/design-notes.md A-6）。 */
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
  .unusable .choice {
    border-color: var(--color-gray-5);
  }
</style>
