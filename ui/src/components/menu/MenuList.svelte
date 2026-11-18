<!-- 一覧（仕様書 2 章「一覧」）。幅 220、地 gray.3、枠 gray.8。行は高さ 24 で、選ぶ印、名前、キー（▸ の行は ▸）。
     上のメニューの行と右クリックの一覧が使う。▸ の子の一覧は、行の右にひらく（入らなければ左）。
     ↑ ↓ で行、Enter で行う、→ で子の一覧（子がなければ onright）、← で onleft。Esc と外を押したときは、ひらいた側がとじる。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import MenuList from "./MenuList.svelte";
  import type { ListEntry, ListRow } from "./menus";

  interface Props {
    entries: readonly ListEntry[];
    /** 行（子の一覧のない行）がえらばれた。ひらいた側がとじてから行う。 */
    onrun: (row: ListRow) => void;
    onleft?: () => void;
    onright?: () => void;
    /** "first": 最初の使える行にフォーカスを置く（キーボードでひらいたとき）。"list": 一覧そのものに置く（↓ で最初の行へ）。 */
    focus: "first" | "list";
    /** 子の一覧か（入らないときは、ずらさずに親の左へ出す）。 */
    nested?: boolean;
    test?: string;
  }
  let { entries, onrun, onleft, onright, focus, nested = false, test }: Props = $props();

  let list = $state<HTMLElement>();
  const rowEls: (HTMLButtonElement | undefined)[] = $state([]);
  let subOpen = $state<number | null>(null);
  let subFocus = $state<"first" | "list">("list");

  const usableRows = () => rowEls.filter((b): b is HTMLButtonElement => !!b && !b.disabled);
  const rowOf = (i: number) => entries[i] as ListRow;

  // 窓からはみ出すときは、窓の中へずらす。子の一覧は、右に入らなければ親の左へ。
  let shift = $state({ x: 0, y: 0 });
  function keepInside(): void {
    if (!list) return;
    const r = list.getBoundingClientRect();
    const left = r.left - shift.x;
    const top = r.top - shift.y;
    let x = Math.max(-left, Math.min(0, window.innerWidth - (left + r.width)));
    if (nested && x < 0) {
      const parent = list.parentElement?.parentElement?.getBoundingClientRect();
      if (parent) x = parent.left - r.width - left;
    }
    const y = Math.max(-top, Math.min(0, window.innerHeight - (top + r.height)));
    if (x !== shift.x || y !== shift.y) shift = { x, y };
  }

  // ひらいた側が一覧を body へ移したあとで、ずらしてからフォーカスを置く（移すとフォーカスが外れるので）。
  onMount(() => {
    queueMicrotask(() => {
      keepInside();
      const first = focus === "first" ? usableRows()[0] : undefined;
      (first ?? list)?.focus();
    });
  });

  function openSub(i: number, byKeys: boolean): void {
    subFocus = byKeys ? "first" : "list";
    subOpen = i;
  }
  function closeSub(): void {
    const i = subOpen;
    subOpen = null;
    if (i !== null) rowEls[i]?.focus();
  }

  function pick(i: number, byKeys: boolean): void {
    const row = rowOf(i);
    if (!row.usable) return;
    if (row.sub) openSub(i, byKeys);
    else onrun(row);
  }

  function onkeydown(ev: KeyboardEvent): void {
    if (ev.altKey || ev.ctrlKey || ev.metaKey || ev.shiftKey || ev.isComposing) return;
    // 子の一覧のキーは、子が受ける。
    const at = rowEls.indexOf(ev.target as HTMLButtonElement);
    if (ev.target !== list && at < 0) return;
    const rows = usableRows();
    const i = at < 0 ? -1 : rows.indexOf(rowEls[at]!);
    if (ev.key === "ArrowDown") {
      rows[i < 0 ? 0 : Math.min(rows.length - 1, i + 1)]?.focus();
    } else if (ev.key === "ArrowUp") {
      rows[i < 0 ? rows.length - 1 : Math.max(0, i - 1)]?.focus();
    } else if (ev.key === "Enter") {
      if (at >= 0) pick(at, true);
    } else if (ev.key === "ArrowRight") {
      if (at >= 0 && rowOf(at).sub && rowOf(at).usable) openSub(at, true);
      else if (onright) onright();
      else return;
    } else if (ev.key === "ArrowLeft") {
      if (onleft) onleft();
      else return;
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
  style:transform={shift.x || shift.y ? `translate(${shift.x}px, ${shift.y}px)` : null}
  bind:this={list}
  {onkeydown}
  data-test={test}
>
  {#each entries as entry, i (i)}
    {#if entry.kind === "separator"}
      <div class="separator" role="separator"></div>
    {:else if entry.kind === "heading"}
      <div class="head" role="presentation"><span class="m-lbl">{entry.label}</span></div>
    {:else}
      <div class="item">
        <button
          class="m-plain row m-focus-in"
          class:unusable={!entry.usable}
          class:expanded={subOpen === i}
          role={entry.checked === undefined ? "menuitem" : "menuitemcheckbox"}
          aria-checked={entry.checked}
          aria-haspopup={entry.sub ? "menu" : undefined}
          aria-expanded={entry.sub ? subOpen === i : undefined}
          disabled={!entry.usable}
          data-row={i}
          bind:this={rowEls[i]}
          onclick={() => pick(i, false)}
          onpointerenter={() => {
            if (entry.sub && entry.usable) openSub(i, false);
            else if (subOpen !== null) subOpen = null;
          }}
        >
          <span class="choice" class:selected={entry.checked === true}><span class="dot"></span></span>
          <span class="label">{entry.label}</span>
          <span class="m-spacer"></span>
          {#if entry.sub}<span class="m-lbl">▸</span>{:else if entry.key}<span class="m-num m-lbl">{entry.key}</span>{/if}
        </button>
        {#if entry.sub && subOpen === i}
          <div class="sub">
            <MenuList entries={entry.sub} {onrun} onleft={closeSub} {onright} focus={subFocus} nested test={test ? `${test}-sub` : undefined} />
          </div>
        {/if}
      </div>
    {/if}
  {/each}
</div>

<style>
  /* 幅は 220。150 % で名前とキーが入りきらない行（「名前をつけて保存… Ctrl Shift S」）があるときだけ、入る幅まで広げる。 */
  .menu {
    width: max-content;
    min-width: var(--part-list-width);
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-8);
    outline: none;
  }
  .item {
    position: relative;
    display: flex;
    flex-direction: column;
  }
  .item:last-child .row {
    border-bottom: 0;
  }
  .head {
    height: var(--size-row);
    display: flex;
    align-items: center;
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    flex-shrink: 0;
  }
  /* 行と行の間のまとまりの区切り（右クリックの一覧）。高さはとらず、行の下の線を gray.6 にする。 */
  .separator {
    flex-shrink: 0;
    border-top: var(--line-hair) solid var(--color-gray-6);
    margin-top: calc(-1 * var(--line-hair));
  }
  .row {
    height: var(--part-list-row);
    border-bottom: var(--line-hair) solid var(--color-gray-4);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    white-space: nowrap;
    flex-shrink: 0;
  }
  /* 乗った ・ 押した ・ 子をひらいている（docs/design-notes.md A-6 の一覧の行と同じ）。 */
  .row:hover:not(:disabled),
  .row.expanded {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .row:hover:not(:disabled) :global(.m-lbl),
  .row.expanded :global(.m-lbl) {
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
  .label {
    color: var(--color-gray-10);
  }
  /* 使えない（仕様書 2 章）: 印の枠 gray.5 ・ 文字 gray.7 ・ 押せない。キーは gray.8 のまま（モック F-Zoom）。 */
  .unusable .label {
    color: var(--color-gray-7);
  }
  .unusable .choice {
    border-color: var(--color-gray-5);
  }
  .unusable .choice.selected .dot {
    background: var(--color-gray-7);
  }
  .sub {
    position: absolute;
    left: 100%;
    top: calc(-1 * var(--line-hair));
    z-index: 1;
  }
</style>
