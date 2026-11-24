<!-- A whole-number value in the chosen clip panel (pitch semitones, cents, the strength of 「音程を寄せる」). Changed by dragging up and down (pressing twice
     calls ontype; the typing box belongs to the panel). While dragging, only the text moves first; one intent is sent on
     release (one drag is one step). Esc stops the drag and goes back to the value before.
     With `reading`, while moving it shows the status line text (dragReading) and a reading in place, at the top left of the cursor. -->
<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { metrics } from "../metrics.svelte";
  import { dragReading, onDragCancel } from "../pianoroll/view.svelte";
  import { dragSteps } from "./clipFields";

  interface Props {
    /** The value in the song. */
    value: number;
    min: number;
    max: number;
    text: (v: number) => string;
    label: string;
    test: string;
    disabled: boolean;
    commit: (v: number) => void;
    ontype: () => void;
    reading?: (v: number) => { status: string; reading: string };
    /** The token whose length the mouse moves for one whole number. */
    step?: string;
  }
  let { value, min, max, text, label, test, disabled, commit, ontype, reading, step = "space.2" }: Props = $props();

  let dragging = $state(false);
  /** The value being moved, or one sent and waiting for the answer. */
  let held = $state<number | null>(null);
  const shown = $derived(held ?? value);
  let fromY = 0;
  let node = $state<HTMLElement>();
  let pointerAt = $state<{ x: number; y: number } | null>(null);
  let readingW = $state(0);
  let readingH = $state(0);
  const shownReading = $derived.by(() => {
    if (!dragging || held === null || held === value || !pointerAt || !reading) return null;
    const off = metrics.px("part.reading.offset");
    return { text: reading(held).reading, left: Math.max(0, pointerAt.x - off - readingW), top: Math.max(0, pointerAt.y - off - readingH) };
  });

  $effect(() => {
    void value;
    untrack(() => {
      if (!dragging) held = null;
    });
  });

  function ondown(e: PointerEvent): void {
    if (e.button !== 0 || disabled) return;
    e.preventDefault();
    node?.setPointerCapture?.(e.pointerId);
    fromY = e.clientY;
    dragging = true;
    held = value;
  }
  function onmove(e: PointerEvent): void {
    if (!dragging) return;
    held = Math.min(max, Math.max(min, value + dragSteps(fromY - e.clientY, metrics.px(step))));
    if (!reading) return;
    pointerAt = { x: e.clientX, y: e.clientY };
    dragReading.text = held === value ? null : reading(held).status;
  }
  function stop(): void {
    dragging = false;
    pointerAt = null;
    if (reading) dragReading.text = null;
  }
  function onup(): void {
    if (!dragging) return;
    stop();
    if (held === null || held === value) {
      held = null;
      return;
    }
    commit(held);
  }
  function cancel(): boolean {
    if (!dragging) return false;
    stop();
    held = null;
    return true;
  }
  onMount(() => onDragCancel(cancel));
  onDestroy(cancel);
</script>

<span
  bind:this={node}
  class="drag"
  class:m-typable={!disabled}
  role="slider"
  tabindex="-1"
  aria-label={label}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={shown}
  data-test={test}
  onpointerdown={ondown}
  onpointermove={onmove}
  onpointerup={onup}
  onpointercancel={cancel}
  ondblclick={() => {
    if (!disabled) ontype();
  }}>{text(shown)}</span
>{#if shownReading}<span
    class="m-num reading"
    style:left="{shownReading.left}px"
    style:top="{shownReading.top}px"
    bind:clientWidth={readingW}
    bind:clientHeight={readingH}
    data-test="{test}-reading">{shownReading.text}</span
  >{/if}

<style>
  .drag {
    cursor: ns-resize;
    touch-action: none;
  }
  /* The reading in place (spec 2; the same form as the arrange reading). Placed against the window so it can go outside the panel. */
  .reading {
    position: fixed;
    z-index: 10;
    height: var(--part-reading-height);
    padding: 0 var(--part-reading-pad-x);
    display: flex;
    align-items: center;
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-8);
    color: var(--color-gray-10);
    white-space: nowrap;
    pointer-events: none;
  }
</style>
