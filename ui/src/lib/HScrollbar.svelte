<!-- 横の棒（仕様書 2 章「スクロールの棒」を横にした物、3 章「横の拡大 ・ 縮小」）。トラックの行やノートの場所のいちばん下に重ねて描く。
     場所はとらない。入りきるとき（visible >= total）は出さない。数は CSS px（棒の長さは visible と同じと見る）。
     高さ 8（上の gray.5 の線 line.hair をふくむ）、地 gray.3、つまみ gray.6、乗ると gray.8、動かしていると gray.9、角 0。
     つまみのいちばん短い長さは棒の太さと同じ 8（縦の棒と同じ。docs/history/design-notes.ja.md A-3）、つまみの外を押すと 1 画面ぶん。
     つまみのドラッグ中の Esc は、動かす前の所へ戻す（仕様書 4 章）。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import { metrics } from "../components/metrics.svelte";
  import { onDragCancel } from "../components/pianoroll/view.svelte";
  import { dragTo, pageTo, thumbOf, type ScrollBox } from "./scrollbar";

  interface Props {
    /** 中身ぜんぶの幅。 */
    total: number;
    /** 見えている幅。 */
    visible: number;
    /** 見えている所の左はし（0 〜 total − visible）。 */
    offset: number;
    /** 人が動かした。新しい offset。 */
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
    // 下の物（クリップをえらぶ、ノートの場所）に押したことを渡さない。
    e.stopPropagation();
    if (e.button !== 0 || !bar) return;
    e.preventDefault();
    onscroll(pageTo(box, e.clientX - bar.getBoundingClientRect().left, minLength));
  }

  /** 棒の上でのホイールは、縦の回しかたでも横に動かす。 */
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
  <!-- マウスだけの部品。キーボードとホイールは、中身の場所がそのまま受ける。 -->
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
    border-top: var(--line-hair) solid var(--color-gray-5);
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
