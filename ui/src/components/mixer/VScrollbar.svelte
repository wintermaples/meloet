<!-- ストリップの縦の棒（仕様書 2 章「スクロールの棒」。lib/HScrollbar.svelte を縦にした物）。ストリップの右はしに重ねて描き、場所はとらない。
     入りきるときは出さない。つまみのドラッグ中の Esc は、動かす前の所へ戻す。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import { dragTo, pageTo, thumbOf, type ScrollBox } from "../../lib/scrollbar";
  import { metrics } from "../metrics.svelte";
  import { onDragCancel } from "../pianoroll/view.svelte";

  let { total, visible, offset, onscroll }: { total: number; visible: number; offset: number; onscroll: (offset: number) => void } = $props();

  const minLength = $derived(metrics.px("part.scrollbar.width"));
  const box: ScrollBox = $derived({ view: visible, content: total, top: offset });
  const thumb = $derived(thumbOf(box, minLength));

  let bar = $state<HTMLDivElement>();
  let dragging = $state(false);
  let drag: { y: number; offset: number; box: ScrollBox } | null = null;

  function onThumbDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    drag = { y: e.clientY, offset, box };
    dragging = true;
  }
  function onThumbMove(e: PointerEvent): void {
    if (drag) onscroll(dragTo(drag.box, drag.offset, e.clientY - drag.y, minLength));
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
    e.stopPropagation();
    if (e.button !== 0 || !bar) return;
    e.preventDefault();
    onscroll(pageTo(box, e.clientY - bar.getBoundingClientRect().top, minLength));
  }
</script>

{#if thumb}
  <!-- マウスだけの部品。ホイールは、ストリップの箱がそのまま受ける。 -->
  <div class="vbar" role="presentation" aria-hidden="true" bind:this={bar} onpointerdown={onBarDown} data-test="mixer-vscrollbar">
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
    ></div>
  </div>
{/if}

<style>
  .vbar {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 5;
    width: var(--part-scrollbar-width);
    background: var(--color-gray-3);
    border-left: var(--line-hair) solid var(--color-gray-5);
  }
  .thumb {
    width: 100%;
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
