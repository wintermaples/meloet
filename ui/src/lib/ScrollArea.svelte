<!-- 縦にスクロールする場所と、スクロールの棒（仕様書 2 章「スクロールの棒」）。ブラウザや OS の棒は出さない。
     つまみのドラッグ中の Esc は、動かす前の所へ戻す（仕様書 4 章）。
     ほかの属性（data-test、data-gl-scroll など）は、スクロールする箱に渡す。 -->
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

  /** つまみのいちばん短い長さ。仕様書にないので、棒の幅と同じにする。 */
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

  /** 棒の上でのホイールも、中身を動かす。 */
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
  <!-- マウスだけの部品。キーボードとホイールは、中身の箱がそのまま受ける。 -->
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
