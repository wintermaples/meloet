<!-- The horizontal bar (spec 2 "Scroll bar" laid on its side; spec 3 "Horizontal zoom in and out"). Drawn over the very bottom
     of the track rows or the note area; it takes no space. Not shown when everything fits (visible >= total). Numbers are
     CSS px (the bar is as long as visible). Height 8 (with the line.thin line.hair on top), background gray.3, handle gray.6,
     gray.8 on hover, gray.9 while moving, corner 0. The shortest handle is as long as the bar is thick, 8 (the same as the
     vertical bar; docs/history/design-notes.ja.md A-3). Pressing outside the handle moves one screen.
     Esc while dragging the handle goes back to where it was (spec 4). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { metrics } from "../components/metrics.svelte";
  import { onDragCancel } from "../components/pianoroll/view.svelte";
  import { dragTo, pageTo, thumbOf, type ScrollBox } from "./scrollbar";

  interface Props {
    /** Width of all the content. */
    total: number;
    /** Width in view. */
    visible: number;
    /** Left end of the part in view (0 to total − visible). */
    offset: number;
    /** The user moved it; the new offset. */
    onscroll: (offset: number) => void;
  }
  let { total, visible, offset, onscroll }: Props = $props();

  const minLength = $derived(metrics.px("part.scrollbar.width"));
  const box: ScrollBox = $derived({ view: visible, content: total, top: offset });
  const thumb = $derived(thumbOf(box, minLength));

  let bar = $state<HTMLDivElement>();
  let dragging = $state(false);
  let drag: { x: number; offset: number; box: ScrollBox } | null = null;

  function onThumbDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    drag = { x: e.clientX, offset, box };
    dragging = true;
  }

  function onThumbMove(e: PointerEvent): void {
    if (!drag) return;
    onscroll(dragTo(drag.box, drag.offset, e.clientX - drag.x, minLength));
  }

  function onThumbUp(): void {
    drag = null;
    dragging = false;
  }

  onMount(() =>
    onDragCancel(() => {
      if (!drag) return false;
      const back = drag.offset;
      onThumbUp();
      onscroll(back);
      return true;
    }),
  );

  function onBarDown(e: PointerEvent): void {
    // Do not pass the press to what is below (choosing clips, the note area).
    e.stopPropagation();
    if (e.button !== 0 || !bar) return;
    e.preventDefault();
    onscroll(pageTo(box, e.clientX - bar.getBoundingClientRect().left, minLength));
  }

  /** The wheel over the bar moves sideways, even when turned vertically. */
  function onWheel(e: WheelEvent): void {
    const d = e.deltaX !== 0 ? e.deltaX : e.deltaY;
    if (d === 0) return;
    e.preventDefault();
    e.stopPropagation();
    const unit = e.deltaMode === 1 ? metrics.px("size.row") : e.deltaMode === 2 ? visible : 1;
    onscroll(Math.min(Math.max(offset + d * unit, 0), Math.max(0, total - visible)));
  }
</script>

{#if thumb}
  <!-- A mouse-only part. The content area takes the keyboard and the wheel itself. -->
  <div
    class="hbar"
    role="presentation"
    aria-hidden="true"
    bind:this={bar}
    onpointerdown={onBarDown}
    onwheel={onWheel}
    data-testid="hscrollbar"
    data-offset={offset}
    data-total={total}
    data-visible={visible}
  >
    <div
      role="presentation"
      class="thumb"
      class:dragging
      style:width="{thumb.length}px"
      style:transform="translateX({thumb.offset}px)"
      onpointerdown={onThumbDown}
      onpointermove={onThumbMove}
      onpointerup={onThumbUp}
      onpointercancel={onThumbUp}
      data-testid="hscrollbar-thumb"
    ></div>
  </div>
{/if}

<style>
  .hbar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 5;
    height: var(--part-scrollbar-width);
    box-sizing: border-box;
    background: var(--color-gray-3);
    border-top: var(--line-hair) solid var(--color-line-thin);
  }
  .thumb {
    height: 100%;
    background: var(--color-gray-6);
    border-radius: var(--radius-0);
  }
  .thumb:hover {
    background: var(--color-gray-8);
  }
  .thumb.dragging {
    background: var(--color-gray-9);
  }
</style>
