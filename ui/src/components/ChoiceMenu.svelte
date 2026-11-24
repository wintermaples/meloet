<!-- List (spec 2 "List", mock F-Menus). Width 220, rows 24 high with a choice mark, a name and a key. Heading rows can be mixed in.
     The chosen row is the row whose value equals selected, or a checked row. A list of actions (the list of song names) leaves the mark empty.
     On open, focus goes to the chosen row (or the first usable row). ↑ ↓ choose, Enter decides. On Esc or a press outside, the opener closes it. -->
<script lang="ts" generics="V">
  import { onMount, type Snippet } from "svelte";

  interface Row {
    value: V;
    label: string;
    /** The key label at the right end. */
    key?: string;
    /** false means disabled (spec 2). */
    usable?: boolean;
    /** Whether it is chosen (for rows that do not use selected). */
    checked?: boolean;
  }
  /** A heading row inside the list. */
  interface Heading {
    heading: string;
  }
  interface Props {
    /** null means no heading row is shown. */
    heading: string | null;
    /** The name when there is no heading (for screen readers). */
    label?: string;
    rows: readonly (Row | Heading)[];
    /** Put at the right end of the heading row. */
    note?: Snippet;
    selected?: V;
    onpick: (value: V) => void;
    test: string;
  }
  let { heading, label, rows, note, selected, onpick, test }: Props = $props();
  const isHeading = (r: Row | Heading): r is Heading => "heading" in r;
  /** undefined means the rows are not choices (action rows). */
  const checkedOf = (r: Row): boolean | undefined => r.checked ?? (selected !== undefined ? r.value === selected : undefined);

  let list = $state<HTMLElement>();
  const buttons = () => [...(list?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];

  // When it goes outside the window, shift it left (up) until it fits. The offset from where the opener put it.
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

  // The opener moves the list to body and places it; shift it after that, then set focus (moving it loses focus).
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
  {#if heading !== null}<div class="head">
      <span class="m-lbl">{heading}</span>{#if note}<span class="m-spacer"></span><span class="m-lbl">{@render note()}</span>{/if}
    </div>{/if}
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
        {#if row.key}<span class="m-spacer"></span><span class="m-num m-lbl key">{row.key}</span>{/if}
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
    border-bottom: var(--line-hair) solid var(--color-line-row);
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
    border-color: var(--color-accent-ink);
  }
  .dot {
    width: var(--part-choice-dot);
    height: var(--part-choice-dot);
  }
  .choice.selected .dot {
    background: var(--color-accent-ink);
  }
  .label {
    color: var(--color-gray-10);
  }
  .row:disabled {
    cursor: default;
  }
  /* Disabled (spec 2): text gray.7, cannot be pressed. The key is gray.7 too. */
  .unusable .label,
  .unusable .key {
    color: var(--color-gray-7);
  }
  .unusable .choice {
    border-color: var(--color-gray-5);
  }
</style>
