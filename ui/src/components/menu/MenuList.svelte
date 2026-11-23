<!-- List (spec 2 "List"). Width 220, bg gray.3, border gray.8. Rows are 24 high with a choice mark, a name and a key (▸ for rows with children).
     Used by the menu bar and the context menu. A ▸ child list opens to the right of the row (to the left if it does not fit).
     ↑ ↓ move between rows, Enter acts, → opens the child list (onright if there is none), ← calls onleft. On Esc or a press outside, the opener closes it. -->
<script lang="ts">
  import { onMount } from "svelte";
  import MenuList from "./MenuList.svelte";
  import type { ListEntry, ListRow } from "./menus";

  interface Props {
    entries: readonly ListEntry[];
    /** A row (without a child list) was chosen. The opener closes the list, then acts. */
    onrun: (row: ListRow) => void;
    onleft?: () => void;
    onright?: () => void;
    /** "first": focus the first usable row (when opened by keyboard). "list": focus the list itself (↓ goes to the first row). */
    focus: "first" | "list";
    /** Whether this is a child list (if it does not fit, it goes to the left of the parent without shifting). */
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

  // When it goes outside the window, shift it inside. A child list goes to the left of the parent if it does not fit on the right.
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

  // The opener moves the list to body; shift it after that, then set focus (moving it loses focus).
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
    // Keys of a child list are handled by the child.
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
          <span class="choice" class:selected={entry.checked === true} class:none={entry.checked === undefined}><span class="dot"></span></span>
          {#if entry.swatch !== undefined}<span class="swatch" style:background="var(--color-track-{entry.swatch})"></span>{/if}
          <span class="label">{entry.label}</span>
          {#if entry.detail}<span class="m-lbl" data-test="menu-detail">{entry.detail}</span>{/if}
          {#if entry.hint}<span class="m-lbl hint" data-test="menu-hint">{entry.hint}</span>{/if}
          <span class="m-spacer"></span>
          {#if entry.sub}<span class="m-lbl key">▸</span>{:else if entry.key}<span class="m-num m-lbl key">{entry.key}</span>{/if}
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
  /* Width 220. Only when a row does not fit its name and key (「名前をつけて保存… Ctrl Shift S」), widen to fit. */
  .menu {
    width: max-content;
    min-width: var(--part-list-width);
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-8);
    padding: var(--space-1) 0;
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
  /* Group separator (spec 2 "List row: added forms"): a line.thin line.hair with space.1 above and below. */
  .separator {
    flex-shrink: 0;
    border-top: var(--line-hair) solid var(--color-line-thin);
    margin: var(--space-1) 0;
  }
  .row {
    height: var(--part-list-row);
    border-bottom: var(--line-hair) solid var(--color-line-row);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    white-space: nowrap;
    flex-shrink: 0;
  }
  /* Hover, pressed, child open (the same as the list row in docs/history/design-notes.ja.md A-6). */
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
  /* Action rows keep the room for the mark but draw no border (mock R5-Menus). */
  .choice.none,
  .unusable .choice.none {
    border-color: transparent;
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
  /* Color sample of a color list row (spec 2 "Color list row"). */
  .swatch {
    flex-shrink: 0;
    width: var(--part-colorlist-swatch-w);
    height: var(--part-colorlist-swatch-h);
  }
  /* Disabled (spec 2): mark border gray.5, text gray.7, cannot be pressed. The key is gray.7 too. */
  .unusable .label,
  .unusable .key {
    color: var(--color-gray-7);
  }
  .unusable .choice {
    border-color: var(--color-gray-5);
  }
  .unusable .choice.selected .dot {
    background: var(--color-gray-7);
  }
  /* The reason it is disabled (「この画面には入りません」) shows only on hover (mock R5-Menus). The width is kept in advance so hovering does not change the list width. */
  .hint {
    visibility: hidden;
  }
  .row:hover .hint {
    visibility: visible;
  }
  /* A child sits space.1 to the right of the parent, with its first row level with the parent row (mock R5-Menus). */
  .sub {
    position: absolute;
    left: calc(100% + var(--space-1));
    top: calc(-1 * (var(--line-hair) + var(--space-1)));
    z-index: 1;
  }
</style>
