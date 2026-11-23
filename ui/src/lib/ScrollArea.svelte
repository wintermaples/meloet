<!-- A vertically scrolling area and its scroll bar (spec 2 "Scroll bar"). The browser's and the OS's bars are not shown.
     Esc while dragging the handle goes back to where it was (spec 4).
     Other attributes (data-test, data-gl-scroll, ...) go to the scrolling box. -->
<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";
  import { metrics } from "../components/metrics.svelte";
  import { onDragCancel } from "../components/pianoroll/view.svelte";
  import { dragTo, pageTo, thumbOf, type ScrollBox, type Thumb } from "./scrollbar";

  let {
    children,
    viewport = $bindable(),
    ...rest
  }: { children: Snippet; viewport?: HTMLDivElement } & HTMLAttributes<HTMLDivElement> = $props();

  let inner = $state<HTMLDivElement>();
  let bar = $state<HTMLDivElement>();
  let thumb = $state<Thumb | null>(null);
  let dragging = $state(false);
  let drag: { y: number; top: number } | null = null;

  /** The shortest handle. Not in the spec, so the same as the bar width. */
  const minLength = () => metrics.px("part.scrollbar.width");

  function box(): ScrollBox | null {
    if (!viewport) return null;
    return { view: viewport.clientHeight, content: viewport.scrollHeight, top: viewport.scrollTop };
  }

  function update(): void {
    const b = box();
    thumb = b ? thumbOf(b, minLength()) : null;
  }

  onMount(() => {
    const observer = new ResizeObserver(() => update());
    if (viewport) observer.observe(viewport);
    if (inner) observer.observe(inner);
    update();
    return () => observer.disconnect();
  });

  function onThumbDown(e: PointerEvent): void {
    if (e.button !== 0 || !viewport) return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    drag = { y: e.clientY, top: viewport.scrollTop };
    dragging = true;
  }

  function onThumbMove(e: PointerEvent): void {
    const b = box();
    if (!drag || !b || !viewport) return;
    viewport.scrollTop = dragTo(b, drag.top, e.clientY - drag.y, minLength());
  }

  function onThumbUp(): void {
    drag = null;
    dragging = false;
  }

  onMount(() =>
    onDragCancel(() => {
      if (!drag) return false;
      const back = drag.top;
      onThumbUp();
      if (viewport) viewport.scrollTop = back;
      return true;
    }),
  );

  function onBarDown(e: PointerEvent): void {
    const b = box();
    if (e.button !== 0 || !b || !viewport || !bar) return;
    e.preventDefault();
    viewport.scrollTop = pageTo(b, e.clientY - bar.getBoundingClientRect().top, minLength());
  }

  /** The wheel over the bar also moves the content. */
  function onBarWheel(e: WheelEvent): void {
    if (!viewport) return;
    e.preventDefault();
    const line = parseFloat(getComputedStyle(viewport).lineHeight) || 0;
    const unit = e.deltaMode === 1 ? line : e.deltaMode === 2 ? viewport.clientHeight : 1;
    viewport.scrollTop += e.deltaY * unit;
  }
</script>

<div class="area">
  <div class="view" bind:this={viewport} onscroll={update} {...rest}>
    <div class="inner" bind:this={inner}>{@render children()}</div>
  </div>
  <!-- A mouse-only part. The content box takes the keyboard and the wheel itself. -->
  <div role="presentation" aria-hidden="true" class="bar" class:hidden={thumb === null} bind:this={bar} onpointerdown={onBarDown} onwheel={onBarWheel} data-test="scrollbar">
    {#if thumb}
      <div
        role="presentation"
        class="thumb"
        class:dragging
        style:height="{thumb.length}px"
        style:transform="translateY({thumb.offset}px)"
        onpointerdown={onThumbDown}
        onpointermove={onThumbMove}
        onpointerup={onThumbUp}
        onpointercancel={onThumbUp}
        data-test="scrollbar-thumb"
      ></div>
    {/if}
  </div>
</div>

<style>
  .area {
    flex: 0 1 auto;
    min-height: 0;
    display: flex;
  }
  .view {
    flex: 1;
    min-width: 0;
    overflow-x: hidden;
    overflow-y: auto;
    scrollbar-width: none;
  }
  .view::-webkit-scrollbar {
    display: none;
  }
  .bar {
    flex-shrink: 0;
    box-sizing: border-box;
    width: var(--part-scrollbar-width);
    background: var(--color-gray-3);
    border-left: var(--line-hair) solid var(--color-gray-5);
  }
  .bar.hidden {
    display: none;
  }
  .thumb {
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
